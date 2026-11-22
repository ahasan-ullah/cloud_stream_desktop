package com.lagradost.cloudstream3.syncproviders.providers

import com.lagradost.cloudstream3.syncproviders.SyncAPI
import com.lagradost.cloudstream3.syncproviders.SyncIdName

open class SimklApi : SyncAPI() {
    override var idPrefix: String = "simkl"
    override var name: String = "Simkl"
    override val syncIdName: SyncIdName = SyncIdName.Simkl
}
