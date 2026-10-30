package com.cloudstream.engine

import java.io.File
import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.nio.file.StandardCopyOption
import java.time.Duration
import kotlin.io.path.moveTo

/** Small HTTP helper for engine housekeeping (repos, plugin downloads). Providers use NiceHttp. */
class Http {
    private val client = HttpClient.newBuilder()
        .followRedirects(HttpClient.Redirect.NORMAL)
        .connectTimeout(Duration.ofSeconds(15))
        .build()

    private val noRedirectClient = HttpClient.newBuilder()
        .followRedirects(HttpClient.Redirect.NEVER)
        .connectTimeout(Duration.ofSeconds(15))
        .build()

    private fun request(url: String) = HttpRequest.newBuilder(URI(url))
        .header("User-Agent", USER_AGENT)
        .timeout(Duration.ofSeconds(60))
        .GET()
        .build()

    fun getText(url: String): String {
        val res = client.send(request(url), HttpResponse.BodyHandlers.ofString())
        if (res.statusCode() !in 200..299) throw ApiError.badGateway("GET $url returned ${res.statusCode()}")
        return res.body()
    }

    /** The `Location` of a redirect response, without following it (used for repo shortcodes). */
    fun redirectLocation(url: String): String? {
        val res = noRedirectClient.send(request(url), HttpResponse.BodyHandlers.discarding())
        return res.headers().firstValue("Location").orElse(null)
    }

    fun download(url: String, target: File) {
        val tmp = File(target.parentFile, target.name + ".part")
        val res = client.send(request(url), HttpResponse.BodyHandlers.ofFile(tmp.toPath()))
        if (res.statusCode() !in 200..299) {
            tmp.delete()
            throw ApiError.badGateway("GET $url returned ${res.statusCode()}")
        }
        tmp.toPath().moveTo(target.toPath(), StandardCopyOption.REPLACE_EXISTING)
    }

    companion object {
        const val USER_AGENT =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36"
    }
}
