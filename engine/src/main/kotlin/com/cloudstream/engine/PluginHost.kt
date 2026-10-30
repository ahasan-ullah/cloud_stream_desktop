package com.cloudstream.engine

import android.content.DesktopContext
import com.googlecode.d2j.dex.Dex2jar
import com.lagradost.cloudstream3.APIHolder
import com.lagradost.cloudstream3.MainAPI
import com.lagradost.cloudstream3.plugins.BasePlugin
import com.lagradost.cloudstream3.plugins.Plugin
import com.lagradost.cloudstream3.utils.extractorApis
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import org.slf4j.LoggerFactory
import java.io.File
import java.net.URLClassLoader
import java.util.concurrent.ConcurrentHashMap
import java.util.zip.ZipFile

@Serializable
private data class PluginManifest(
    val pluginClassName: String? = null,
    val name: String? = null,
    val version: Int? = null,
    val requiresResources: Boolean = false,
)

/**
 * Loads CloudStream plugins (.cs3) on the desktop JVM.
 *
 * A .cs3 file is a zip with `manifest.json` and Android `classes.dex`. The DEX is
 * translated to a JVM jar once (cached), loaded in its own class loader, and the
 * plugin's `load()` registers its providers into CloudStream's global APIHolder.
 */
class PluginHost(
    private val pluginDir: File,
    private val cacheDir: File,
    private val http: Http,
) {
    private class Loaded(
        val id: String,
        val file: File,
        val manifest: PluginManifest?,
        val instance: BasePlugin?,
        val loader: URLClassLoader?,
        val error: String?,
    )

    private val log = LoggerFactory.getLogger("PluginHost")
    private val json = Json { ignoreUnknownKeys = true }
    private val plugins = ConcurrentHashMap<String, Loaded>()

    init {
        pluginDir.mkdirs()
        cacheDir.mkdirs()
    }

    fun loadAll() {
        pluginDir.listFiles { f -> f.isFile && f.extension in PLUGIN_EXTENSIONS }
            ?.sortedBy { it.name }
            ?.forEach { load(it) }
        log.info("Loaded {} plugin(s), {} provider(s)", plugins.size, APIHolder.allProviders.size)
    }

    fun list(): List<PluginDto> = plugins.values.sortedBy { it.id.lowercase() }.map { it.toDto() }

    @Synchronized
    fun install(url: String, id: String?): PluginDto {
        val pluginId = sanitize(id ?: url.substringBefore('?').substringAfterLast('/').substringBeforeLast('.'))
        if (pluginId.isEmpty()) throw ApiError.badRequest("Cannot derive a plugin id from $url")
        unload(pluginId)
        val target = File(pluginDir, "$pluginId.cs3")
        http.download(url, target)
        load(target)
        return plugins.getValue(pluginId).toDto()
    }

    @Synchronized
    fun uninstall(id: String) {
        val loaded = plugins[id] ?: throw ApiError.notFound("Plugin $id is not installed")
        unload(id)
        loaded.file.delete()
        deleteCachedJars(id)
    }

    /** Versioned so jars from an older converter setup are regenerated. */
    private fun cachedJar(id: String) = File(cacheDir, "$id.d2j$CONVERTER_VERSION.jar")

    private fun deleteCachedJars(id: String) {
        cacheDir.listFiles { f -> f.name == "$id.jar" || f.name.startsWith("$id.d2j") }?.forEach { it.delete() }
    }

    private fun load(file: File) {
        val id = file.nameWithoutExtension
        plugins[id] = try {
            loadUnchecked(id, file)
        } catch (t: Throwable) {
            // LinkageErrors are expected for plugins that touch Android-only APIs.
            log.warn("Failed to load plugin {}: {}", file.name, t.toString())
            Loaded(id, file, null, null, null, t.toString())
        }
    }

    private fun loadUnchecked(id: String, file: File): Loaded {
        val (manifest, jar) = ZipFile(file).use { zip ->
            val manifestEntry = zip.getEntry("manifest.json") ?: error("manifest.json missing")
            val manifest = json.decodeFromString<PluginManifest>(
                zip.getInputStream(manifestEntry).readBytes().decodeToString()
            )
            val dex = zip.getEntry("classes.dex")
            val jar = if (dex == null) {
                file // Already JVM bytecode.
            } else {
                cachedJar(id).also { out ->
                    if (!out.exists() || out.lastModified() < file.lastModified()) {
                        deleteCachedJars(id)
                        Dex2jar.from(zip.getInputStream(dex).readBytes())
                            // Keep JVM names such as Kotlin's `constructor-impl` intact; renaming them breaks linking.
                            .dontSanitizeNames(true)
                            .to(out.toPath())
                    }
                }
            }
            manifest to jar
        }
        val className = manifest.pluginClassName ?: error("manifest.json has no pluginClassName")

        val loader = URLClassLoader(arrayOf(jar.toURI().toURL()), javaClass.classLoader)
        try {
            val instance = loader.loadClass(className).getDeclaredConstructor().newInstance() as BasePlugin
            instance.filename = file.absolutePath
            if (instance is Plugin) instance.load(DesktopContext) else instance.load()
            log.info("Loaded plugin {} -> {}", file.name, providersOf(file).map { it.name })
            return Loaded(id, file, manifest, instance, loader, null)
        } catch (t: Throwable) {
            removeProviders(file)
            loader.close()
            throw t
        }
    }

    private fun unload(id: String) {
        val loaded = plugins.remove(id) ?: return
        runCatching { loaded.instance?.beforeUnload() }
        removeProviders(loaded.file)
        runCatching { loaded.loader?.close() }
    }

    private fun removeProviders(file: File) {
        val path = file.absolutePath
        providersOf(file).forEach {
            APIHolder.removePluginMapping(it)
            APIHolder.allProviders.remove(it)
        }
        extractorApis.removeAll { it.sourcePlugin == path }
    }

    private fun providersOf(file: File): List<MainAPI> =
        APIHolder.allProviders.filter { it.sourcePlugin == file.absolutePath }

    private fun Loaded.toDto() = PluginDto(
        id = id,
        name = manifest?.name ?: id,
        version = manifest?.version,
        providers = providersOf(file).map { it.name },
        error = error,
    )

    private fun sanitize(name: String) = name.replace(Regex("[^A-Za-z0-9._-]"), "_").trim('.', '_')

    companion object {
        private val PLUGIN_EXTENSIONS = setOf("cs3", "jar")
        private const val CONVERTER_VERSION = 2
    }
}
