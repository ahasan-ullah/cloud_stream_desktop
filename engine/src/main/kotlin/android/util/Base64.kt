package android.util

import java.util.Base64 as JBase64

/** android.util.Base64 on top of java.util.Base64, keeping Android's flags and line wrapping. */
object Base64 {
    const val DEFAULT = 0
    const val NO_PADDING = 1
    const val NO_WRAP = 2
    const val CRLF = 4
    const val URL_SAFE = 8
    const val NO_CLOSE = 16

    @JvmStatic
    fun decode(str: String, flags: Int): ByteArray = decode(str.toByteArray(Charsets.US_ASCII), flags)

    @JvmStatic
    fun decode(input: ByteArray, flags: Int): ByteArray = decode(input, 0, input.size, flags)

    /** Like Android: ignores whitespace, padding is optional, throws IllegalArgumentException on bad input. */
    @JvmStatic
    fun decode(input: ByteArray, offset: Int, len: Int, flags: Int): ByteArray {
        val text = String(input, offset, len, Charsets.US_ASCII).filterNot { it.isWhitespace() }.trimEnd('=')
        val std = if (flags and URL_SAFE != 0) text.replace('-', '+').replace('_', '/') else text
        return JBase64.getDecoder().decode(std + "=".repeat((4 - std.length % 4) % 4))
    }

    @JvmStatic
    fun encode(input: ByteArray, flags: Int): ByteArray = encodeToString(input, flags).toByteArray(Charsets.US_ASCII)

    @JvmStatic
    fun encode(input: ByteArray, offset: Int, len: Int, flags: Int): ByteArray =
        encodeToString(input, offset, len, flags).toByteArray(Charsets.US_ASCII)

    @JvmStatic
    fun encodeToString(input: ByteArray, flags: Int): String = encodeToString(input, 0, input.size, flags)

    @JvmStatic
    fun encodeToString(input: ByteArray, offset: Int, len: Int, flags: Int): String {
        if (len == 0) return ""
        val bytes = input.copyOfRange(offset, offset + len)
        val encoder = if (flags and URL_SAFE != 0) JBase64.getUrlEncoder() else JBase64.getEncoder()
        var out = encoder.encodeToString(bytes)
        if (flags and NO_PADDING != 0) out = out.trimEnd('=')
        if (flags and NO_WRAP == 0) {
            val newline = if (flags and CRLF != 0) "\r\n" else "\n"
            out = out.chunked(76).joinToString(newline) + newline
        }
        return out
    }
}
