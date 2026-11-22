package android.graphics

object Color {
    const val BLACK = -0x1000000
    const val DKGRAY = -0xbbbbbc
    const val GRAY = -0x777778
    const val LTGRAY = -0x333334
    const val WHITE = -0x1
    const val RED = -0x10000
    const val GREEN = -0xff0100
    const val BLUE = -0xffff01
    const val YELLOW = -0x100
    const val CYAN = -0xff0001
    const val MAGENTA = -0xff01
    const val TRANSPARENT = 0

    @JvmStatic
    fun parseColor(colorString: String): Int = try {
        java.awt.Color.decode(colorString).rgb
    } catch (_: Throwable) {
        0
    }
}
