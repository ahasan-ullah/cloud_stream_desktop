package com.cloudstream.engine

import kotlinx.serialization.json.Json
import org.slf4j.LoggerFactory
import java.io.File
import java.net.URI

/** Persists the user's extension repositories and reads their plugin lists. */
class RepoStore(private val file: File, private val http: Http) {
    private val log = LoggerFactory.getLogger("RepoStore")
    private val json = Json { ignoreUnknownKeys = true; prettyPrint = true }
    private val repos = mutableListOf<RepoDto>()

    init {
        if (file.exists()) {
            runCatching { repos += json.decodeFromString<List<RepoDto>>(file.readText()) }
                .onFailure { log.warn("Ignoring unreadable {}: {}", file, it.toString()) }
        }
    }

    @Synchronized
    fun list(): List<RepoDto> = repos.toList()

    @Synchronized
    fun add(input: String): RepoDto {
        val url = resolveRepoUrl(input) ?: throw ApiError.badRequest("Invalid repository URL or shortcode: $input")
        val manifest = fetchManifest(url)
        val repo = RepoDto(url = url, name = manifest.name, description = manifest.description)
        repos.removeAll { it.url == url }
        repos += repo
        save()
        return repo
    }

    @Synchronized
    fun remove(url: String) {
        repos.removeAll { it.url == url }
        save()
    }

    fun plugins(url: String): List<RepoPluginDto> {
        val manifest = fetchManifest(url)
        return manifest.pluginLists.flatMap { listUrl ->
            val resolved = URI(url).resolve(listUrl).toString()
            runCatching { json.decodeFromString<List<RepoPluginDto>>(http.getText(resolved)) }
                .onFailure { log.warn("Plugin list {} failed: {}", resolved, it.toString()) }
                .getOrDefault(emptyList())
        }.distinctBy { it.internalName }
    }

    /**
     * Same inputs the Android app accepts (RepositoryManager.parseRepoUrl): a URL, a
     * `cloudstreamrepo://` / `https://cs.repo/` link, or a cutt.ly (`code`) / py.md (`!code`) shortcode.
     */
    private fun resolveRepoUrl(input: String): String? {
        val text = input.trim()
        val deepLink = Regex("^(cloudstreamrepo://|https://cs\\.repo/\\??)")
        return when {
            deepLink.containsMatchIn(text) -> text.replace(deepLink, "").let {
                if (it.startsWith("http://") || it.startsWith("https://")) it else "https://$it"
            }
            Regex("^https?://").containsMatchIn(text) -> text
            text.matches(Regex("^[a-zA-Z0-9!_-]+$")) -> {
                val (host, code) = if (text.startsWith("!")) "https://py.md" to text.removePrefix("!") else "https://cutt.ly" to text
                runCatching { http.redirectLocation("$host/$code") }.getOrNull()
                    ?.takeUnless { it.startsWith("$host/404") || it.removeSuffix("/") == host }
            }
            else -> null
        }
    }

    private fun fetchManifest(url: String): RepoManifest = try {
        json.decodeFromString<RepoManifest>(http.getText(url))
    } catch (e: ApiError) {
        throw e
    } catch (e: Exception) {
        throw ApiError.badRequest("$url is not a CloudStream repository: ${e.message}")
    }

    private fun save() {
        file.parentFile?.mkdirs()
        file.writeText(json.encodeToString(repos.toList()))
    }
}
