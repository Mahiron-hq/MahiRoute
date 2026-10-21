// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.res

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Test

/** Keeps every translation complete: a string added in English must also exist in Russian. */
class StringResourcesTest {
    private fun names(folder: String): Set<String> {
        val file = File("src/main/res/$folder/strings.xml")
        val document = javax.xml.parsers.DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(file)
        val nodes = document.getElementsByTagName("string")
        return (0 until nodes.length).map { nodes.item(it).attributes.getNamedItem("name").nodeValue }.toSet()
    }

    @Test
    fun russianHasEveryString() {
        assertEquals(names("values"), names("values-ru"))
    }
}
