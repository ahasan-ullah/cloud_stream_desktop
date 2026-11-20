package com.lagradost.cloudstream3.plugins

import com.cloudstream.engine.RepoStore
import com.lagradost.cloudstream3.ui.settings.extensions.RepositoryData
import org.slf4j.LoggerFactory

object RepositoryManager {
    private val log = LoggerFactory.getLogger("RepositoryManager")

    @Volatile
    var repoStore: RepoStore? = null

    fun getRepositories(): Array<RepositoryData> {
        val store = repoStore ?: return emptyArray()
        val list = store.list().map {
            RepositoryData(name = it.name, url = it.url)
        }
        return list.toTypedArray()
    }

    suspend fun parseRepository(url: String): Repository? {
        val store = repoStore ?: return null
        return try {
            val resolvedUrl = store.resolveRepoUrl(url) ?: url
            val manifest = store.fetchManifest(resolvedUrl)
            Repository(name = manifest.name, url = resolvedUrl, iconUrl = null)
        } catch (e: Exception) {
            log.warn("RepositoryManager: Failed to parse repository {}: {}", url, e.toString())
            null
        }
    }

    suspend fun addRepository(repository: RepositoryData) {
        val store = repoStore ?: return
        try {
            store.add(repository.url)
            log.info("RepositoryManager: Added repository '{}' ({})", repository.name, repository.url)
        } catch (e: Exception) {
            log.warn("RepositoryManager: Failed to add repository {}: {}", repository.url, e.toString())
        }
    }

    suspend fun addRepository(url: String) {
        val store = repoStore ?: return
        try {
            store.add(url)
            log.info("RepositoryManager: Added repository by URL ({})", url)
        } catch (e: Exception) {
            log.warn("RepositoryManager: Failed to add repository {}: {}", url, e.toString())
        }
    }

    suspend fun removeRepository(repository: RepositoryData) {
        repoStore?.remove(repository.url)
    }

    suspend fun removeRepository(url: String) {
        repoStore?.remove(url)
    }
}
