package com.cloudstream.engine

import android.content.DesktopContext
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
    plugins.loadAll()

    val server = embeddedServer(CIO, host = "127.0.0.1", port = port) {
        engineModule(plugins, repos, token)
    }.start(wait = false)

    val boundPort = runBlocking { server.engine.resolvedConnectors().first().port }
    println("ENGINE_READY port=$boundPort")
    System.out.flush()
    IdleTrimmer.touch() // Plugin loading allocated a lot; trim once startup settles.
    IdleTrimmer.start()

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
