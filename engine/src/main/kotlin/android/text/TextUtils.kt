package android.text

object TextUtils {
    @JvmStatic
    fun isEmpty(str: CharSequence?): Boolean = str.isNullOrEmpty()

    @JvmStatic
    fun join(delimiter: CharSequence, tokens: Iterable<*>): String = tokens.joinToString(delimiter)

    @JvmStatic
    fun join(delimiter: CharSequence, tokens: Array<Any?>): String = tokens.joinToString(delimiter)

    @JvmStatic
    fun equals(a: CharSequence?, b: CharSequence?): Boolean = a?.toString() == b?.toString()
}
