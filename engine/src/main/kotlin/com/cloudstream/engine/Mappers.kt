package com.cloudstream.engine

import com.lagradost.cloudstream3.AnimeLoadResponse
import com.lagradost.cloudstream3.AnimeSearchResponse
import com.lagradost.cloudstream3.Episode
import com.lagradost.cloudstream3.LiveStreamLoadResponse
import com.lagradost.cloudstream3.LoadResponse
import com.lagradost.cloudstream3.MainAPI
import com.lagradost.cloudstream3.MovieLoadResponse
import com.lagradost.cloudstream3.SearchResponse
import com.lagradost.cloudstream3.SubtitleFile
import com.lagradost.cloudstream3.TvSeriesLoadResponse
import com.lagradost.cloudstream3.utils.ExtractorLink

fun MainAPI.toDto() = ProviderDto(
    name = name,
    mainUrl = mainUrl,
    lang = lang,
    hasMainPage = hasMainPage,
    supportedTypes = supportedTypes.map { it.name },
    plugin = sourcePlugin,
)

fun SearchResponse.toDto() = SearchItemDto(
    name = name,
    url = url,
    apiName = apiName,
    type = type?.name,
    posterUrl = posterUrl,
    posterHeaders = posterHeaders,
    quality = quality?.name,
    dubStatus = (this as? AnimeSearchResponse)?.dubStatus?.map { it.name },
)

fun Episode.toDto(dubStatus: String? = null) = EpisodeDto(
    data = data,
    name = name,
    season = season,
    episode = episode,
    posterUrl = posterUrl,
    description = description,
    dubStatus = dubStatus,
    runTime = runTime,
    date = date,
)

fun LoadResponse.toDto(): DetailsDto {
    val episodes = when (this) {
        is TvSeriesLoadResponse -> episodes.map { it.toDto() }
        is AnimeLoadResponse -> episodes.flatMap { (dub, list) -> list.map { it.toDto(dub.name) } }
        else -> emptyList()
    }
    val playData = when (this) {
        is MovieLoadResponse -> dataUrl
        is LiveStreamLoadResponse -> dataUrl
        else -> null
    }
    return DetailsDto(
        name = name,
        url = url,
        apiName = apiName,
        type = type.name,
        posterUrl = posterUrl,
        backgroundPosterUrl = backgroundPosterUrl,
        year = year,
        plot = plot,
        tags = tags.orEmpty(),
        duration = duration,
        playData = playData,
        episodes = episodes.distinctBy { it.dubStatus to it.data },
        recommendations = recommendations.orEmpty().map { it.toDto() }.distinctBy { it.url },
        rating = score?.toDouble(10)?.takeIf { it > 0 },
        contentRating = contentRating,
        comingSoon = comingSoon,
        actors = actors.orEmpty().map { a ->
            ActorDto(a.actor.name, a.actor.image, a.roleString ?: a.role?.name?.lowercase()?.replaceFirstChar { it.uppercase() })
        }.distinctBy { it.name },
        trailers = trailers.orEmpty().map { it.extractorUrl }.distinct(),
    )
}

fun ExtractorLink.toDto(): LinkDto {
    val merged = LinkedHashMap(headers)
    if (referer.isNotBlank() && merged.keys.none { it.equals("referer", ignoreCase = true) }) {
        merged["Referer"] = referer
    }
    return LinkDto(
        name = name,
        source = source,
        url = url,
        quality = quality,
        type = type.name,
        headers = merged,
    )
}

fun SubtitleFile.toDto() = SubtitleDto(lang = lang, url = url, headers = headers.orEmpty())
