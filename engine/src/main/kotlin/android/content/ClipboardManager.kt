package android.content

class ClipboardManager {
    fun setPrimaryClip(clip: ClipData?) {}
    fun getPrimaryClip(): ClipData? = null
}

class ClipData {
    companion object {
        @JvmStatic
        fun newPlainText(label: CharSequence?, text: CharSequence?): ClipData = ClipData()
    }
}
