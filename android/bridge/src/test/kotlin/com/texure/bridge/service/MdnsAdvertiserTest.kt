package com.texure.bridge.service

import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class MdnsAdvertiserTest {

    @Test
    fun testServiceConstants() {
        assertEquals("_texure._tcp.local.", MdnsAdvertiser.SERVICE_TYPE)
        assertEquals(8765, MdnsAdvertiser.BRIDGE_PORT)
        assertEquals("0.1.0", MdnsAdvertiser.VERSION)
    }

    @Test
    fun testServiceNameUsesAppId() {
        assertEquals("Texure-com.texure.sample", MdnsAdvertiser.serviceName("com.texure.sample"))
    }

    @Test
    fun testTxtRecordsCarryVersionAndAppId() {
        val txt = MdnsAdvertiser.txtRecords("com.texure.sample")
        assertEquals("0.1.0", txt["version"])
        assertEquals("com.texure.sample", txt["appId"])
    }

    @Test
    fun testBuildServiceInfoMatchesContract() {
        val info = MdnsAdvertiser.buildServiceInfo("com.texure.sample")
        assertEquals("Texure-com.texure.sample", info.serviceName)
        assertEquals("_texure._tcp.local.", info.serviceType)
        assertEquals(8765, info.port)
        assertEquals("0.1.0", String(info.attributes["version"]!!, Charsets.UTF_8))
        assertEquals("com.texure.sample", String(info.attributes["appId"]!!, Charsets.UTF_8))
    }
}
