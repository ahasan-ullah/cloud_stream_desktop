package android.widget

import android.content.Context

class Toast(val context: Context? = null) {
    fun show() {}

    companion object {
        const val LENGTH_SHORT = 0
        const val LENGTH_LONG = 1

        @JvmStatic
        fun makeText(context: Context?, text: CharSequence?, duration: Int): Toast = Toast(context)

        @JvmStatic
        fun makeText(context: Context?, resId: Int, duration: Int): Toast = Toast(context)
    }
}
