package com.texure.bridge

import android.app.Activity
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import com.texure.bridge.injection.DefaultInjectionAdapter
import com.texure.bridge.injection.InjectionResult
import com.texure.bridge.model.Key
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class InjectionAdapterTest {

    private lateinit var activity: Activity
    private lateinit var button: Button
    private lateinit var editText: EditText
    private lateinit var adapter: DefaultInjectionAdapter

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        val layout = LinearLayout(activity)
        button = Button(activity).apply {
            text = "Button"
        }
        editText = EditText(activity).apply {
            hint = "Input"
        }
        layout.addView(button)
        layout.addView(editText)
        activity.setContentView(layout)
        layout.measure(
            android.view.View.MeasureSpec.makeMeasureSpec(1080, android.view.View.MeasureSpec.EXACTLY),
            android.view.View.MeasureSpec.makeMeasureSpec(1920, android.view.View.MeasureSpec.EXACTLY)
        )
        layout.layout(0, 0, 1080, 1920)

        adapter = DefaultInjectionAdapter(activity)
    }

    @Test
    fun testTapDispatchesSuccessfully() {
        var clicked = false
        button.setOnClickListener { clicked = true }

        val location = IntArray(2)
        button.getLocationOnScreen(location)
        val x = location[0] + button.width / 2f
        val y = location[1] + button.height / 2f

        val result = adapter.tap(x, y)
        assertEquals(InjectionResult.Success, result)
    }

    @Test
    fun testTextTypesIntoEditable() {
        val result = adapter.text(editText, "typed text")
        assertEquals(InjectionResult.Success, result)
        assertEquals("typed text", editText.text.toString())
    }

    @Test
    fun testKeyDispatchesSuccessfully() {
        val result = adapter.key(Key.ENTER)
        assertEquals(InjectionResult.Success, result)
    }
}
