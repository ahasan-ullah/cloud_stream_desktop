package android.webkit

class CookieManager {
    fun getCookie(url: String?): String? = null
    fun setCookie(url: String?, value: String?) {}
    fun removeAllCookies(callback: Any? = null) {}
    fun flush() {}

    companion object {
        private val instance = CookieManager()

        @JvmStatic
        fun getInstance(): CookieManager = instance
    }
}
