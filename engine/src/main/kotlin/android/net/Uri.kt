package android.net

import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder

/** The commonly used read-only part of android.net.Uri. */
open class Uri private constructor(private val raw: String) {
    private val parsed: URI? = runCatching { URI(raw) }.getOrNull()

    open fun getScheme(): String? = parsed?.scheme ?: raw.substringBefore(':', "").ifEmpty { null }
    open fun getHost(): String? = parsed?.host
    open fun getPort(): Int = parsed?.port ?: -1
    open fun getAuthority(): String? = parsed?.authority
    open fun getEncodedAuthority(): String? = parsed?.rawAuthority
    open fun getPath(): String? = parsed?.path
    open fun getEncodedPath(): String? = parsed?.rawPath
    open fun getQuery(): String? = parsed?.query
    open fun getEncodedQuery(): String? = parsed?.rawQuery
    open fun getFragment(): String? = parsed?.fragment
    open fun getEncodedFragment(): String? = parsed?.rawFragment
    open fun getSchemeSpecificPart(): String? = parsed?.schemeSpecificPart
    open fun isAbsolute(): Boolean = getScheme() != null
    open fun isRelative(): Boolean = !isAbsolute()

    open fun getPathSegments(): List<String> = getPath().orEmpty().split('/').filter { it.isNotEmpty() }
    open fun getLastPathSegment(): String? = getPathSegments().lastOrNull()

    private fun queryPairs(): List<Pair<String, String>> =
        getEncodedQuery().orEmpty().split('&').filter { it.isNotEmpty() }.map {
            decode(it.substringBefore('=')).orEmpty() to decode(it.substringAfter('=', "")).orEmpty()
        }

    open fun getQueryParameter(key: String?): String? = queryPairs().firstOrNull { it.first == key }?.second
    open fun getQueryParameters(key: String?): List<String> = queryPairs().filter { it.first == key }.map { it.second }
    open fun getQueryParameterNames(): Set<String> = queryPairs().map { it.first }.toCollection(LinkedHashSet())
    open fun getBooleanQueryParameter(key: String?, defaultValue: Boolean): Boolean =
        getQueryParameter(key)?.let { it != "false" && it != "0" } ?: defaultValue

    override fun toString(): String = raw
    override fun equals(other: Any?): Boolean = other is Uri && other.raw == raw
    override fun hashCode(): Int = raw.hashCode()

    companion object {
        @JvmField
        val EMPTY: Uri = Uri("")

        @JvmStatic
        fun parse(uriString: String?): Uri = Uri(uriString.orEmpty())

        @JvmStatic
        fun encode(s: String?): String? = encode(s, null)

        /** Android leaves `-_.!~*'()` and the allowed characters unescaped and uses %20 for spaces. */
        @JvmStatic
        fun encode(s: String?, allow: String?): String? {
            if (s == null) return null
            val keep = "-_.!~*'()" + allow.orEmpty()
            return buildString {
                for (ch in s) {
                    if (ch.isLetterOrDigit() && ch.code < 128 || ch in keep) append(ch)
                    else append(URLEncoder.encode(ch.toString(), Charsets.UTF_8).replace("+", "%20"))
                }
            }
        }

        @JvmStatic
        fun decode(s: String?): String? =
            s?.let { runCatching { URLDecoder.decode(it.replace("+", "%2B"), Charsets.UTF_8) }.getOrDefault(it) }
    }
}
