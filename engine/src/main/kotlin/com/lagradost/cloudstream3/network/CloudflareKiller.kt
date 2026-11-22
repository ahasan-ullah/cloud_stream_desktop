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
open class CloudflareKiller : Interceptor {
    val savedCookies: MutableMap<String, Map<String, String>> = mutableMapOf()

    override fun intercept(chain: Interceptor.Chain): Response {
        return chain.proceed(chain.request())
    }
}
