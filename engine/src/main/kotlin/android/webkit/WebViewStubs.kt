package android.webkit

import android.content.Context

open class WebView(context: Context? = null) {
    open fun getSettings(): WebSettings = WebSettings()
    open fun setWebViewClient(client: WebViewClient) {}
    open fun loadUrl(url: String) {}
    open fun loadUrl(url: String, additionalHttpHeaders: Map<String, String>) {}
    open fun stopLoading() {}
    open fun destroy() {}
}

open class WebSettings {
    var javaScriptEnabled: Boolean = true
    var domStorageEnabled: Boolean = true
    var userAgentString: String? = null
}

open class WebViewClient {
    open fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean = false
    open fun onPageFinished(view: WebView?, url: String?) {}
}

open class WebResourceRequest {
    open fun getUrl(): android.net.Uri? = null
    open fun getMethod(): String = "GET"
    open fun getRequestHeaders(): Map<String, String> = emptyMap()
}

open class WebResourceResponse(
    val mimeType: String?,
    val encoding: String?,
    val data: java.io.InputStream?
)
