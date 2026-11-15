package com.lagradost.cloudstream3.utils

import android.content.Context
import android.content.DesktopContext
import android.content.SharedPreferences
import kotlinx.serialization.json.Json

/**
 * Desktop shim for CloudStream's DataStore helper.
 * Plugins use this to read and write preferences and settings.
 */
object DataStore {
    fun getSharedPrefs(context: Context?): SharedPreferences =
        context?.getSharedPreferences("default", Context.MODE_PRIVATE)
            ?: DesktopContext.preferences("default")

    fun <T> setKey(path: String, value: T) {
        DesktopContext.preferences("default").edit().putString(path, value?.toString()).apply()
    }

    fun <T> setKey(tag: String, key: String, value: T) {
        DesktopContext.preferences(tag).edit().putString(key, value?.toString()).apply()
    }

    @Suppress("UNCHECKED_CAST")
    fun <T> getKey(path: String): T? =
        DesktopContext.preferences("default").getString(path, null) as? T

    @Suppress("UNCHECKED_CAST")
    fun <T> getKey(tag: String, key: String): T? =
        DesktopContext.preferences(tag).getString(key, null) as? T

    fun removeKey(path: String) {
        DesktopContext.preferences("default").edit().remove(path).apply()
    }

    fun removeKey(tag: String, key: String) {
        DesktopContext.preferences(tag).edit().remove(key).apply()
    }

    fun containsKey(path: String): Boolean =
        DesktopContext.preferences("default").contains(path)

    fun containsKey(tag: String, key: String): Boolean =
        DesktopContext.preferences(tag).contains(key)
}

object DataStoreHelper {
    fun <T> setKey(path: String, value: T) = DataStore.setKey(path, value)
    fun <T> setKey(tag: String, key: String, value: T) = DataStore.setKey(tag, key, value)
    fun <T> getKey(path: String): T? = DataStore.getKey(path)
    fun <T> getKey(tag: String, key: String): T? = DataStore.getKey(tag, key)
    fun removeKey(path: String) = DataStore.removeKey(path)
    fun removeKey(tag: String, key: String) = DataStore.removeKey(tag, key)
}
