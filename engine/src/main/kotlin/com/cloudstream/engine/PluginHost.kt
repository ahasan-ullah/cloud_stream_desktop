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
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Opcodes
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
                        val tmp = File(cacheDir, "$id.d2j$CONVERTER_VERSION.jar.tmp")
                        tmp.delete()
                        try {
                            Dex2jar.from(zip.getInputStream(dex).readBytes())
                                // Keep JVM names such as Kotlin's `constructor-impl` intact; renaming them breaks linking.
                                .dontSanitizeNames(true)
                                .to(tmp.toPath())
                            if (!tmp.renameTo(out)) {
                                tmp.copyTo(out, overwrite = true)
                                tmp.delete()
                            }
                        } finally {
                            tmp.delete()
                        }
                    }
                }
            }
            manifest to jar
        }
        val className = manifest.pluginClassName ?: error("manifest.json has no pluginClassName")

        val loader = ComposeAwareClassLoader(arrayOf(jar.toURI().toURL()), javaClass.classLoader)
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
        private const val CONVERTER_VERSION = 3
    }
}

/**
 * URLClassLoader that transparently patches missing Compose stability fields and references.
 *
 * The Kotlin Compose compiler inserts `public static final int $stable = 0;` into every
 * class that carries a @Composable or @Stable annotation. It also compiles bytecode that
 * reads `$stable` from superclasses (e.g. `Plugin.$stable`). When converted to JVM bytecode,
 * those references survive, but the Compose runtime itself is not on the engine's classpath,
 * causing `NoSuchFieldError: $stable`.
 *
 * This loader intercepts every class load from the plugin jar and:
 *  1. Replaces any `GETSTATIC *.$stable : I` instruction with `ICONST_0` so reads never fail.
 *  2. Declares `public static final int $stable = 0;` on classes that reference it if missing.
 */
private class ComposeAwareClassLoader(urls: Array<java.net.URL>, parent: ClassLoader?) :
    URLClassLoader(urls, parent) {

    private val patchCache = ConcurrentHashMap<String, ByteArray>()

    override fun findClass(name: String): Class<*> {
        synchronized(getClassLoadingLock(name)) {
            val cached = patchCache[name]
            if (cached != null) return defineClass(name, cached, 0, cached.size)
            val patched = readAndPatch(name)
                ?: return super.findClass(name)
            patchCache[name] = patched
            return defineClass(name, patched, 0, patched.size)
        }
    }

    private fun readAndPatch(name: String): ByteArray? {
        val path = name.replace('.', '/') + ".class"
        val raw = findResource(path)?.openStream()?.use { it.readBytes() }
            ?: getResourceAsStream(path)?.use { it.readBytes() }
            ?: return null
        return injectStableFieldIfNeeded(raw)
    }

    companion object {
        private const val STABLE_FIELD = "\$stable"
        private const val FIELD_DESC = "I"

        fun injectStableFieldIfNeeded(raw: ByteArray): ByteArray {
            if (!referencesStable(raw)) return raw

            val cr = ClassReader(raw)
            val hasField = hasStableField(cr)
            val cw = ClassWriter(cr, 0)
            val cv = object : org.objectweb.asm.ClassVisitor(Opcodes.ASM9, cw) {
                private var fieldInjected = false

                override fun visitMethod(
                    access: Int, name: String?, descriptor: String?, signature: String?, exceptions: Array<out String>?
                ): org.objectweb.asm.MethodVisitor? {
                    val mv = super.visitMethod(access, name, descriptor, signature, exceptions) ?: return null
                    return object : org.objectweb.asm.MethodVisitor(Opcodes.ASM9, mv) {
                        override fun visitFieldInsn(opcode: Int, owner: String?, fieldName: String?, fieldDesc: String?) {
                            if (opcode == Opcodes.GETSTATIC && fieldName == STABLE_FIELD && fieldDesc == FIELD_DESC) {
                                // Pushes 0 instead of reading the static field from another class.
                                super.visitInsn(Opcodes.ICONST_0)
                            } else {
                                super.visitFieldInsn(opcode, owner, fieldName, fieldDesc)
                            }
                        }
                    }
                }

                override fun visitEnd() {
                    if (!hasField && !fieldInjected) {
                        fieldInjected = true
                        visitField(
                            Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC or Opcodes.ACC_FINAL,
                            STABLE_FIELD, FIELD_DESC, null, 0
                        )?.visitEnd()
                    }
                    super.visitEnd()
                }
            }
            cr.accept(cv, 0)
            return cw.toByteArray()
        }

        private fun hasStableField(cr: ClassReader): Boolean {
            var found = false
            cr.accept(object : org.objectweb.asm.ClassVisitor(Opcodes.ASM9) {
                override fun visitField(access: Int, name: String, desc: String, sig: String?, value: Any?): org.objectweb.asm.FieldVisitor? {
                    if (name == STABLE_FIELD) found = true
                    return null
                }
            }, ClassReader.SKIP_CODE or ClassReader.SKIP_DEBUG or ClassReader.SKIP_FRAMES)
            return found
        }

        private fun referencesStable(raw: ByteArray): Boolean {
            val needle = STABLE_FIELD.toByteArray(Charsets.UTF_8)
            outer@ for (i in 0..(raw.size - needle.size)) {
                for (j in needle.indices) if (raw[i + j] != needle[j]) continue@outer
                return true
            }
            return false
        }
    }
}
