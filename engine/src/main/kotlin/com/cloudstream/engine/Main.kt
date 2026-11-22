package com.cloudstream.engine

import android.content.DesktopContext
import com.fasterxml.jackson.core.JsonParser
import com.fasterxml.jackson.databind.*
import com.fasterxml.jackson.databind.introspect.AnnotatedMember
import com.fasterxml.jackson.databind.introspect.AnnotationIntrospectorPair
import com.fasterxml.jackson.databind.module.SimpleDeserializers
import com.fasterxml.jackson.databind.module.SimpleModule
import com.lagradost.cloudstream3.APIHolder
import com.lagradost.cloudstream3.mapper
import com.lagradost.cloudstream3.plugins.RepositoryManager
import io.ktor.server.cio.CIO
import io.ktor.server.engine.embeddedServer
import kotlinx.coroutines.runBlocking
import java.io.File
import kotlin.concurrent.thread
import kotlin.system.exitProcess

/**
 * Headless CloudStream engine.
 *
 * Environment:
 *  - CS_ENGINE_DATA         data directory (plugins, repos.json, caches)
 *  - CS_ENGINE_TOKEN        shared secret required in the X-Engine-Token header
 *  - CS_ENGINE_PORT         port to bind; 0 (default) picks a free one
 *  - CS_ENGINE_WATCH_STDIN  "1" to exit when stdin closes (parent app died)
 *
 * Prints `ENGINE_READY port=<port>` on stdout once requests can be served.
 */
fun main() {
    val dataDir = File(
        System.getenv("CS_ENGINE_DATA") ?: File(System.getProperty("user.home"), ".cloudstream-desktop").path
    ).apply { mkdirs() }
    val token = System.getenv("CS_ENGINE_TOKEN").orEmpty()
    val port = System.getenv("CS_ENGINE_PORT")?.toIntOrNull() ?: 0
    DesktopContext.dataDir = dataDir

    val http = Http()
    val plugins = PluginHost(File(dataDir, "plugins"), File(dataDir, "cache/plugins"), http)
    val repos = RepoStore(File(dataDir, "repos.json"), http)
    RepositoryManager.repoStore = repos

    // Wrap Jackson's AnnotationIntrospector so Kotlin reflection errors on local/synthetic classes
    // (such as in MegaPlugin) are swallowed rather than crashing JSON deserialization.
    try {
        val original = mapper.serializationConfig.annotationIntrospector
        if (original != null) {
            val safe = object : AnnotationIntrospectorPair(original, AnnotationIntrospector.nopInstance()) {
                override fun hasRequiredMarker(m: AnnotatedMember): Boolean? = try {
                    _primary.hasRequiredMarker(m)
                } catch (_: Throwable) {
                    null
                }

                override fun findDefaultCreator(
                    config: com.fasterxml.jackson.databind.cfg.MapperConfig<*>,
                    valueClass: com.fasterxml.jackson.databind.introspect.AnnotatedClass,
                    declaredConstructors: MutableList<com.fasterxml.jackson.databind.introspect.PotentialCreator>,
                    declaredFactories: MutableList<com.fasterxml.jackson.databind.introspect.PotentialCreator>
                ): com.fasterxml.jackson.databind.introspect.PotentialCreator? = try {
                    _primary.findDefaultCreator(config, valueClass, declaredConstructors, declaredFactories)
                } catch (_: Throwable) {
                    null
                }

                override fun findImplicitPropertyName(m: AnnotatedMember): String? = try {
                    _primary.findImplicitPropertyName(m)
                } catch (_: Throwable) {
                    null
                }
            }
            mapper.setAnnotationIntrospector(safe)
        }

        val sm = SimpleModule().apply {
            setDeserializers(object : SimpleDeserializers() {
                override fun findBeanDeserializer(
                    type: JavaType,
                    config: DeserializationConfig,
                    beanDesc: BeanDescription
                ): JsonDeserializer<*>? {
                    if (type.rawClass.name.endsWith("VerifiedRepo")) {
                        return object : JsonDeserializer<Any>() {
                            override fun deserialize(p: JsonParser, ctxt: DeserializationContext): Any? {
                                val node = p.codec.readTree<JsonNode>(p)
                                val url = node.get("url")?.asText()
                                val verified = node.get("verified")?.asBoolean()
                                val ctor = type.rawClass.constructors.firstOrNull { it.parameterTypes.size == 2 }
                                return ctor?.newInstance(url, verified)
                            }
                        }
                    }
                    return null
                }
            })
        }
        mapper.registerModule(sm)
    } catch (e: Throwable) {
        // Safe fallback if mapper internals change
    }

    val server = embeddedServer(CIO, host = "127.0.0.1", port = port) {
        engineModule(plugins, repos, token)
    }.start(wait = false)

    val boundPort = runBlocking { server.engine.resolvedConnectors().first().port }
    println("ENGINE_READY port=$boundPort")
    System.out.flush()

    thread(isDaemon = true, name = "plugin-loader") {
        plugins.loadAll()
        println("ENGINE_PLUGINS_LOADED count=${APIHolder.allProviders.size}")
        System.out.flush()
        IdleTrimmer.touch() // Plugin loading allocated a lot; trim once startup settles.
        IdleTrimmer.start()
    }

    if (System.getenv("CS_ENGINE_WATCH_STDIN") == "1") {
        thread(isDaemon = true, name = "stdin-watch") {
            // Blocks until the parent app closes our stdin (or dies).
            var read = 0
            while (read != -1) read = System.`in`.read()
            server.stop(200, 1_000)
            exitProcess(0)
        }
    }
    Thread.currentThread().join()
}
