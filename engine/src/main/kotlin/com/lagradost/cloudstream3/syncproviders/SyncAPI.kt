package com.lagradost.cloudstream3.syncproviders

import com.lagradost.cloudstream3.utils.UiText

/**
 * Desktop stub for SyncAPI and related classes.
 * Providers like CineStream reference these to interact with watchlist/sync services.
 */
abstract class SyncAPI {
    open var idPrefix: String = ""
    open var name: String = ""
    open val syncIdName: SyncIdName = SyncIdName.LocalList

    open class LibraryList(
        open val name: UiText = UiText.Txt(""),
        open val items: List<Any> = emptyList()
    )

    open class LibraryMetadata(
        open val allLibraryLists: List<LibraryList> = emptyList()
    )
}

data class AuthUser(
    val username: String = "",
    val id: String = ""
)

open class SyncRepo(val api: SyncAPI) {
    open fun authUser(): AuthUser? = null
    open suspend fun library(): Result<SyncAPI.LibraryMetadata> = Result.success(SyncAPI.LibraryMetadata())
}

class AccountManager {
    companion object {
        val simklApi = com.lagradost.cloudstream3.syncproviders.providers.SimklApi()
    }
}
