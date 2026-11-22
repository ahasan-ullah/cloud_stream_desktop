package com.lagradost.cloudstream3.utils

open class Event<T> {
    private val listeners = mutableListOf<(T) -> Unit>()

    open operator fun plusAssign(observer: (T) -> Unit) {
        listeners.add(observer)
    }

    open fun invoke(data: T) {
        listeners.forEach { it(data) }
    }
}
