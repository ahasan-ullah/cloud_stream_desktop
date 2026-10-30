package com.cloudstream.engine

import kotlinx.serialization.Serializable

@Serializable
data class ErrorDto(val error: String)

@Serializable
data class ProviderDto(
    val name: String,
    val mainUrl: String,
    val lang: String,
    val hasMainPage: Boolean,
    val supportedTypes: List<String>,
    val plugin: String?,
)

@Serializable
data class SearchItemDto(
    val name: String,
    val url: String,
    val apiName: String,
    val type: String? = null,
    val posterUrl: String? = null,
    val posterHeaders: Map<String, String>? = null,
    val quality: String? = null,
    /** "Dubbed"/"Subbed" flags shown as badges on anime cards. */
    val dubStatus: List<String>? = null,
)

@Serializable
data class HomeSectionDto(val name: String, val items: List<SearchItemDto>, val horizontal: Boolean)

@Serializable
data class HomeDto(
    val provider: String,
    val sections: List<HomeSectionDto>,
    val hasNext: Boolean,
    val errors: List<String>,
    /** Number of home categories the provider has; fetch them in slices with from/count. */
    val total: Int,
)

@Serializable
data class SearchGroupDto(val provider: String, val items: List<SearchItemDto>, val error: String? = null)

@Serializable
data class EpisodeDto(
    val data: String,
    val name: String? = null,
    val season: Int? = null,
    val episode: Int? = null,
    val posterUrl: String? = null,
    val description: String? = null,
    val dubStatus: String? = null,
    /** Minutes. */
    val runTime: Int? = null,
    /** Air date, epoch milliseconds. */
    val date: Long? = null,
)

@Serializable
data class DetailsDto(
    val name: String,
    val url: String,
    val apiName: String,
    val type: String? = null,
    val posterUrl: String? = null,
    val backgroundPosterUrl: String? = null,
    val year: Int? = null,
    val plot: String? = null,
    val tags: List<String> = emptyList(),
    val duration: Int? = null,
    /** Set for movies and live streams: pass straight to /api/links. */
    val playData: String? = null,
    val episodes: List<EpisodeDto> = emptyList(),
    val recommendations: List<SearchItemDto> = emptyList(),
    /** Score out of 10. */
    val rating: Double? = null,
    val contentRating: String? = null,
    val comingSoon: Boolean = false,
    val actors: List<ActorDto> = emptyList(),
    /** Trailer page URLs (usually YouTube). */
    val trailers: List<String> = emptyList(),
)

@Serializable
data class ActorDto(val name: String, val image: String? = null, val role: String? = null)

@Serializable
data class LinkDto(
    val name: String,
    val source: String,
    val url: String,
    val quality: Int,
    /** VIDEO, M3U8, DASH, TORRENT or MAGNET. */
    val type: String,
    val headers: Map<String, String>,
)

@Serializable
data class SubtitleDto(val lang: String, val url: String, val headers: Map<String, String> = emptyMap())

@Serializable
data class LinksDto(val links: List<LinkDto>, val subtitles: List<SubtitleDto>)

@Serializable
data class PluginDto(
    val id: String,
    val name: String,
    val version: Int? = null,
    val providers: List<String>,
    val error: String? = null,
)

@Serializable
data class InstallRequest(val url: String, val id: String? = null)

@Serializable
data class IdRequest(val id: String)

@Serializable
data class UrlRequest(val url: String)

/** Repository manifest (`repo.json`) as published by CloudStream extension repos. */
@Serializable
data class RepoManifest(
    val name: String,
    val description: String? = null,
    val manifestVersion: Int = 1,
    val pluginLists: List<String> = emptyList(),
)

@Serializable
data class RepoDto(val url: String, val name: String, val description: String? = null)

/** One entry of a repository's `plugins.json`. */
@Serializable
data class RepoPluginDto(
    val url: String,
    val name: String,
    val internalName: String,
    val version: Int = 0,
    val status: Int = 1,
    val description: String? = null,
    val authors: List<String> = emptyList(),
    val language: String? = null,
    val iconUrl: String? = null,
    val tvTypes: List<String>? = null,
    val fileSize: Long? = null,
)
