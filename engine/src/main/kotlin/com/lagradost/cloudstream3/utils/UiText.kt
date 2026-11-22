package com.lagradost.cloudstream3.utils

import android.content.Context

open class UiText {
    open fun asString(context: Context? = null): String = ""

    open class Txt(val text: String) : UiText() {
        override fun asString(context: Context?): String = text
    }

    open class DynamicString(val value: String) : UiText() {
        override fun asString(context: Context?): String = value
    }

    open class StringResource(val resId: Int, val args: List<Any> = emptyList()) : UiText() {
        override fun asString(context: Context?): String = "Res($resId)"
    }
}
