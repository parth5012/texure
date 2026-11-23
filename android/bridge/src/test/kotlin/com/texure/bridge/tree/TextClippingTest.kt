package com.texure.bridge.tree

import android.app.Activity
import android.text.TextUtils
import android.view.View
import android.widget.FrameLayout
import android.widget.TextView
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TextClippingTest {

    private lateinit var activity: Activity
    private lateinit var extractor: BridgeTreeExtractor

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        extractor = BridgeTreeExtractor()
    }

    @Test
    fun testUnclippedTextViewHasClippedFalse() {
        val root = FrameLayout(activity)
        val textView = TextView(activity).apply {
            text = "Short text"
            id = View.generateViewId()
        }
        root.addView(textView)
        activity.setContentView(root)

        // Measure and layout
        root.measure(
            View.MeasureSpec.makeMeasureSpec(500, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(200, View.MeasureSpec.EXACTLY)
        )
        root.layout(0, 0, 500, 200)

        val tree = extractor.extractTree(activity)
        val node = tree.nodes.firstOrNull { it.text == "Short text" }
        assertNotNull(node)
        assertEquals(false, node?.clipped)
    }

    @Test
    fun testEllipsizedTextViewHasClippedTrue() {
        val root = FrameLayout(activity)
        val textView = TextView(activity).apply {
            text = "This is a very long text that will surely be ellipsized because the width is constrained to a tiny container"
            ellipsize = TextUtils.TruncateAt.END
            maxLines = 1
            id = View.generateViewId()
        }
        root.addView(textView)
        activity.setContentView(root)

        // Measure with tight width (50px)
        root.measure(
            View.MeasureSpec.makeMeasureSpec(50, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(40, View.MeasureSpec.EXACTLY)
        )
        root.layout(0, 0, 50, 40)

        val tree = extractor.extractTree(activity)
        val node = tree.nodes.firstOrNull { it.text?.startsWith("This is a very long") == true }
        assertNotNull(node)
        assertEquals(true, node?.clipped)
    }

    @Test
    fun testTextOverflowBoundsHasClippedTrue() {
        val root = FrameLayout(activity)
        val textView = TextView(activity).apply {
            text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6"
            id = View.generateViewId()
        }
        root.addView(textView)
        activity.setContentView(root)

        // Measured height constrained to 20px so layout height exceeds view height
        textView.measure(
            View.MeasureSpec.makeMeasureSpec(300, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(20, View.MeasureSpec.EXACTLY)
        )
        textView.layout(0, 0, 300, 20)

        val tree = extractor.extractTree(activity)
        val node = tree.nodes.firstOrNull { it.text?.startsWith("Line 1") == true }
        assertNotNull(node)
        assertEquals(true, node?.clipped)
    }

    private fun assertNotNull(obj: Any?) {
        assertTrue(obj != null)
    }
}
