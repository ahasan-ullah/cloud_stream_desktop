package com.lagradost.cloudstream3.network

import okhttp3.Interceptor
import okhttp3.Response

/**
 * Desktop stub for CloudflareKiller.
 *
 * On Android, this interceptor drives a WebView to pass Cloudflare challenges.
 * On desktop, we provide this binary-compatible stub so providers referencing it
 * load and execute without NoClassDefFoundError.
 */
open class CloudflareKiller(val context: Any? = null) : Interceptor {
    constructor() : this(null)

    val savedCookies: MutableMap<String, Map<String, String>> = mutableMapOf()

    fun getCookie(url: String): Map<String, String>? = savedCookies[url]

    override fun intercept(chain: Interceptor.Chain): Response {
        return chain.proceed(chain.request())
    }
}
