package com.texure.companion

import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.PowerManager
import androidx.test.core.app.ApplicationProvider
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [Build.VERSION_CODES.UPSIDE_DOWN_CAKE]) // Android 14
class CompanionDaemonServiceTest {

    private lateinit var context: Context

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        CompanionDaemonService.isRunning = false
        CompanionDaemonService.isWakeLockHeld = false
        CompanionDaemonService.currentStatus = "Stopped"
    }

    @After
    fun tearDown() {
        CompanionDaemonService.isRunning = false
        CompanionDaemonService.isWakeLockHeld = false
        CompanionDaemonService.currentStatus = "Stopped"
    }

    @Test
    fun testNotificationChannelCreated() {
        CompanionDaemonService.createNotificationChannel(context)

        val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val channel = manager.getNotificationChannel(CompanionDaemonService.CHANNEL_ID)

        assertNotNull("Channel should be created", channel)
        assertEquals(CompanionDaemonService.CHANNEL_NAME, channel.name)
        assertEquals(NotificationManager.IMPORTANCE_LOW, channel.importance)
    }

    @Test
    fun testBuildNotificationContainsActionsAndStatus() {
        val notification = CompanionDaemonService.buildNotification(context, "Connected to PC (192.168.1.10)")

        assertNotNull(notification)
        assertEquals(CompanionDaemonService.CHANNEL_ID, notification.channelId)

        // Verify ongoing flag
        assertTrue((notification.flags and android.app.Notification.FLAG_ONGOING_EVENT) != 0)

        // Verify actions: Stop Relay and Restart App
        assertNotNull(notification.actions)
        assertEquals(2, notification.actions.size)
        assertEquals("Stop Relay", notification.actions[0].title.toString())
        assertEquals("Restart App", notification.actions[1].title.toString())

        // Verify content text
        val extras = notification.extras
        assertEquals("Texure Relay", extras.getString(android.app.Notification.EXTRA_TITLE))
        assertEquals("Texure Relay: Connected to PC (192.168.1.10)", extras.getString(android.app.Notification.EXTRA_TEXT))
    }

    @Test
    fun testServiceStartWithoutWakeLock() {
        val intent = Intent(context, CompanionDaemonService::class.java).apply {
            action = CompanionDaemonService.ACTION_START
            putExtra(CompanionDaemonService.EXTRA_STATUS, "Connected to PC")
            putExtra(CompanionDaemonService.EXTRA_ACQUIRE_WAKE_LOCK, false)
        }

        val controller = Robolectric.buildService(CompanionDaemonService::class.java, intent)
        controller.create().startCommand(0, 1)

        assertTrue(CompanionDaemonService.isRunning)
        assertEquals("Connected to PC", CompanionDaemonService.currentStatus)
        assertFalse(CompanionDaemonService.isWakeLockHeld)

        controller.destroy()
        assertFalse(CompanionDaemonService.isRunning)
    }

    @Test
    fun testServiceStartWithWakeLock() {
        val intent = Intent(context, CompanionDaemonService::class.java).apply {
            action = CompanionDaemonService.ACTION_START
            putExtra(CompanionDaemonService.EXTRA_STATUS, "Active Agent Session")
            putExtra(CompanionDaemonService.EXTRA_ACQUIRE_WAKE_LOCK, true)
        }

        val controller = Robolectric.buildService(CompanionDaemonService::class.java, intent)
        val service = controller.create().startCommand(0, 1).get()

        assertTrue(CompanionDaemonService.isRunning)
        assertEquals("Active Agent Session", CompanionDaemonService.currentStatus)
        assertTrue(CompanionDaemonService.isWakeLockHeld)

        // Stop via ACTION_STOP
        val stopIntent = Intent(context, CompanionDaemonService::class.java).apply {
            action = CompanionDaemonService.ACTION_STOP
        }
        controller.startCommand(0, 2)
        service.onStartCommand(stopIntent, 0, 2)

        assertFalse(CompanionDaemonService.isWakeLockHeld)
        assertFalse(CompanionDaemonService.isRunning)
        assertEquals("Stopped", CompanionDaemonService.currentStatus)

        controller.destroy()
    }

    @Test
    fun testServiceUpdateStatus() {
        val startIntent = Intent(context, CompanionDaemonService::class.java).apply {
            action = CompanionDaemonService.ACTION_START
        }
        val controller = Robolectric.buildService(CompanionDaemonService::class.java, startIntent)
        val service = controller.create().startCommand(0, 1).get()

        assertEquals("Connected to PC", CompanionDaemonService.currentStatus)

        val updateIntent = Intent(context, CompanionDaemonService::class.java).apply {
            action = CompanionDaemonService.ACTION_UPDATE_STATUS
            putExtra(CompanionDaemonService.EXTRA_STATUS, "Syncing Screenshots")
        }
        service.onStartCommand(updateIntent, 0, 2)

        assertEquals("Syncing Screenshots", CompanionDaemonService.currentStatus)

        controller.destroy()
    }

    @Test
    fun testServiceStopRelayAction() {
        val startIntent = Intent(context, CompanionDaemonService::class.java).apply {
            action = CompanionDaemonService.ACTION_START
        }
        val controller = Robolectric.buildService(CompanionDaemonService::class.java, startIntent)
        val service = controller.create().startCommand(0, 1).get()

        assertTrue(CompanionDaemonService.isRunning)

        val stopRelayIntent = Intent(context, CompanionDaemonService::class.java).apply {
            action = CompanionDaemonService.ACTION_STOP_RELAY
        }
        service.onStartCommand(stopRelayIntent, 0, 2)

        assertFalse(CompanionDaemonService.isRunning)
        assertEquals("Stopped", CompanionDaemonService.currentStatus)

        controller.destroy()
    }

    @Test
    fun testWakeLockAcquisitionAndRelease() {
        val controller = Robolectric.buildService(CompanionDaemonService::class.java)
        val service = controller.create().get()

        assertFalse(CompanionDaemonService.isWakeLockHeld)
        service.acquireWakeLock()
        assertTrue(CompanionDaemonService.isWakeLockHeld)

        // Multiple acquire should remain held
        service.acquireWakeLock(10000L)
        assertTrue(CompanionDaemonService.isWakeLockHeld)

        service.releaseWakeLock()
        assertFalse(CompanionDaemonService.isWakeLockHeld)

        // Safe multiple release
        service.releaseWakeLock()
        assertFalse(CompanionDaemonService.isWakeLockHeld)

        controller.destroy()
    }
}
