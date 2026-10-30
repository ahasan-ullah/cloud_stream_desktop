package com.cloudstream.engine

import com.lagradost.cloudstream3.APIHolder
import com.lagradost.cloudstream3.MainAPI
import com.lagradost.cloudstream3.MainPageRequest
import io.ktor.http.HttpStatusCode
import io.ktor.serialization.kotlinx.json.json
import io.ktor.server.application.Application
import io.ktor.server.application.ApplicationCall
import io.ktor.server.application.ApplicationCallPipeline
import io.ktor.server.application.call
import io.ktor.server.application.install
import io.ktor.server.plugins.contentnegotiation.ContentNegotiation
import io.ktor.server.plugins.statuspages.StatusPages
import io.ktor.server.request.path
import io.ktor.server.request.receive
import io.ktor.server.response.respond
import io.ktor.server.routing.get
import io.ktor.server.routing.post
import io.ktor.server.routing.route
import io.ktor.server.routing.routing
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.Json
import org.slf4j.LoggerFactory
import java.util.Collections

class ApiError(val status: HttpStatusCode, override val message: String) : RuntimeException(message) {
    companion object {
        fun badRequest(msg: String) = ApiError(HttpStatusCode.BadRequest, msg)
        fun notFound(msg: String) = ApiError(HttpStatusCode.NotFound, msg)
        fun badGateway(msg: String) = ApiError(HttpStatusCode.BadGateway, msg)
    }
}

private const val SEARCH_TIMEOUT_MS = 25_000L
private const val HOME_TIMEOUT_MS = 30_000L
private const val LINKS_TIMEOUT_MS = 60_000L

private val log = LoggerFactory.getLogger("Routes")

private fun ApplicationCall.param(name: String): String =
    request.queryParameters[name]?.takeIf { it.isNotBlank() } ?: throw ApiError.badRequest("Missing '$name'")

private fun providers(): List<MainAPI> = APIHolder.allProviders.toList()

private fun provider(name: String): MainAPI =
    providers().firstOrNull { it.name == name } ?: throw ApiError.notFound("Provider '$name' is not installed")

fun Application.engineModule(plugins: PluginHost, repos: RepoStore, token: String) {
    install(ContentNegotiation) {
        json(Json { explicitNulls = false; encodeDefaults = true; ignoreUnknownKeys = true })
    }
    install(StatusPages) {
        exception<ApiError> { call, e -> call.respond(e.status, ErrorDto(e.message)) }
        exception<Throwable> { call, e ->
            log.error("Unhandled error on {}", call.request.path(), e)
            call.respond(HttpStatusCode.InternalServerError, ErrorDto(e.message ?: e.toString()))
        }
    }

    // Only the desktop app (which knows the token) may drive the engine.
    if (token.isNotEmpty()) {
        intercept(ApplicationCallPipeline.Plugins) {
            if (call.request.path() != "/health" && call.request.headers["X-Engine-Token"] != token) {
                call.respond(HttpStatusCode.Unauthorized, ErrorDto("Invalid engine token"))
                finish()
            }
        }
    }
    intercept(ApplicationCallPipeline.Monitoring) { IdleTrimmer.touch() }

    routing {
        get("/health") { call.respond(mapOf("status" to "ok")) }

        route("/api") {
            get("/providers") { call.respond(providers().map { it.toDto() }) }

            get("/home") {
                val api = provider(call.param("provider"))
                val page = call.request.queryParameters["page"]?.toIntOrNull() ?: 1
                // Categories are fetched in slices so the app only scrapes what is about to be shown.
                val from = call.request.queryParameters["from"]?.toIntOrNull()?.coerceAtLeast(0) ?: 0
                val count = call.request.queryParameters["count"]?.toIntOrNull()?.coerceAtLeast(1) ?: Int.MAX_VALUE
                val all = api.mainPage.map { MainPageRequest(it.name, it.data, it.horizontalImages) }
                    .ifEmpty { listOf(MainPageRequest("", "", false)) }
                val requests = all.drop(from).take(count)

                val results = coroutineScope {
                    requests.map { req ->
                        async(Dispatchers.IO) {
                            req to runCatching { withTimeout(HOME_TIMEOUT_MS) { api.getMainPage(page, req) } }
                        }
                    }.awaitAll()
                }
                val sections = results.flatMap { (req, result) ->
                    result.getOrNull()?.items.orEmpty().map { list ->
                        HomeSectionDto(
                            name = list.name.ifBlank { req.name },
                            items = list.list.map { it.toDto() }.distinctBy { it.url },
                            horizontal = req.horizontalImages,
                        )
                    }
                }.filter { it.items.isNotEmpty() }
                val errors = results.mapNotNull { (req, result) ->
                    result.exceptionOrNull()?.let {
                        log.warn("{} home section '{}' failed: {}", api.name, req.name, it.toString())
                        "${req.name.ifBlank { api.name }}: ${it.message ?: it}"
                    }
                }
                if (sections.isEmpty() && requests.isNotEmpty()) {
                    log.warn("{} home: {} request(s), no sections, {} error(s)", api.name, requests.size, errors.size)
                }
                val hasNext = results.any { it.second.getOrNull()?.hasNext == true }
                call.respond(HomeDto(api.name, sections, hasNext, errors, all.size))
            }

            get("/search") {
                val query = call.param("query")
                val only = call.request.queryParameters["provider"]
                val targets = if (only.isNullOrBlank()) providers() else listOf(provider(only))
                val groups = coroutineScope {
                    targets.map { api ->
                        async(Dispatchers.IO) {
                            try {
                                // The paged overload is what newer plugins implement; its default falls
                                // back to search(query) for older ones, same as the Android app.
                                val page = withTimeout(SEARCH_TIMEOUT_MS) { api.search(query, 1) }
                                // Plugins often list the same title twice; the UI keys rows by URL.
                                SearchGroupDto(api.name, page?.items.orEmpty().map { it.toDto() }.distinctBy { it.url })
                            } catch (t: Throwable) {
                                log.warn("{} search '{}' failed: {}", api.name, query, t.toString())
                                SearchGroupDto(api.name, emptyList(), t.message ?: t.toString())
                            }
                        }
                    }.awaitAll()
                }
                call.respond(groups)
            }

            get("/load") {
                val api = provider(call.param("provider"))
                val url = call.param("url")
                val details = withContext(Dispatchers.IO) { api.load(url) }
                    ?: throw ApiError.notFound("${api.name} returned nothing for $url")
                call.respond(details.toDto())
            }

            get("/links") {
                val api = provider(call.param("provider"))
                val data = call.param("data")
                val links = Collections.synchronizedList(mutableListOf<LinkDto>())
                val subtitles = Collections.synchronizedList(mutableListOf<SubtitleDto>())
                withContext(Dispatchers.IO) {
                    // Keep whatever was found if a slow extractor hits the timeout.
                    withTimeoutOrNull(LINKS_TIMEOUT_MS) {
                        api.loadLinks(data, false, { subtitles += it.toDto() }, { links += it.toDto() })
                    }
                }
                call.respond(
                    LinksDto(
                        links = links.toList().distinctBy { it.url }.sortedByDescending { it.quality },
                        subtitles = subtitles.toList().distinctBy { it.url },
                    )
                )
            }

            route("/plugins") {
                get { call.respond(plugins.list()) }
                post("/install") {
                    val req = call.receive<InstallRequest>()
                    call.respond(withContext(Dispatchers.IO) { plugins.install(req.url, req.id) })
                }
                post("/uninstall") {
                    val req = call.receive<IdRequest>()
                    withContext(Dispatchers.IO) { plugins.uninstall(req.id) }
                    call.respond(plugins.list())
                }
            }

            route("/repos") {
                get { call.respond(repos.list()) }
                get("/plugins") {
                    val url = call.param("url")
                    call.respond(withContext(Dispatchers.IO) { repos.plugins(url) })
                }
                post("/add") {
                    val req = call.receive<UrlRequest>()
                    call.respond(withContext(Dispatchers.IO) { repos.add(req.url) })
                }
                post("/remove") {
                    repos.remove(call.receive<UrlRequest>().url)
                    call.respond(repos.list())
                }
            }
        }
    }
}
