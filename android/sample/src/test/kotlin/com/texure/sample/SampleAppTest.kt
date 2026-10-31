package com.texure.sample

import android.widget.Button
import android.widget.EditText
import android.widget.TextView
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SampleAppTest {

    private lateinit var activity: MainActivity

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(MainActivity::class.java).setup().get()
    }

    @Test
    fun testViewsExistInMainActivity() {
        val button = activity.findViewById<Button>(R.id.btn_test)
        val input = activity.findViewById<EditText>(R.id.et_input)
        val status = activity.findViewById<TextView>(R.id.tv_status)

        assertNotNull("Button must exist", button)
        assertNotNull("EditText must exist", input)
        assertNotNull("Status TextView must exist", status)
    }

    @Test
    fun testButtonClickUpdatesStatus() {
        val button = activity.findViewById<Button>(R.id.btn_test)
        val status = activity.findViewById<TextView>(R.id.tv_status)

        assertEquals("Clicks: 0", status.text.toString())
        button.performClick()
        assertEquals("Clicks: 1", status.text.toString())
    }
}
