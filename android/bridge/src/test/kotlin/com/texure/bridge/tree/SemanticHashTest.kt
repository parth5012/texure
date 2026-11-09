package com.texure.bridge.tree

import android.app.Activity
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Map #36 #29: stable semantic node hashes.
 *
 * A node's `hash` is deterministic (FNV-1a 64 over ancestor path, stable
 * tag/resource id, text snippet, class name): identical re-extractions and
 * rev bumps reproduce it, non-structural updates elsewhere leave it alone,
 * and the node's own identity change (text/id/class/ancestry) rotates it.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SemanticHashTest {

    private lateinit var activity: Activity
    private lateinit var testButton: Button
    private lateinit var testInput: EditText
    private lateinit var layout: LinearLayout

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        layout = LinearLayout(activity)

        testButton = Button(activity).apply {
            id = 101
            text = "Click Me"
            contentDescription = "Test button"
        }
        testInput = EditText(activity).apply {
            id = 102
        }
        testInput.setText("hello")

        layout.addView(testButton)
        layout.addView(testInput)
        activity.setContentView(layout)
        layout.measure(
            android.view.View.MeasureSpec.makeMeasureSpec(1080, android.view.View.MeasureSpec.EXACTLY),
            android.view.View.MeasureSpec.makeMeasureSpec(1920, android.view.View.MeasureSpec.EXACTLY)
        )
        layout.layout(0, 0, 1080, 1920)
        org.robolectric.shadows.ShadowLooper.idleMainLooper()
    }

    private fun hashOf(tree: ExtractedTree, text: String): String {
        val node = tree.nodes.firstOrNull { it.text == text }
            ?: error("no node with text $text: ${tree.nodes.map { it.text }}")
        return node.hash ?: error("node $text has no hash")
    }

    @Test
    fun everyInterestingNodeCarriesWellFormedHash() {
        val tree = BridgeTreeExtractor().extractTree(activity)
        assertTrue("expected nodes", tree.nodes.size >= 2)
        for (node in tree.nodes) {
            val hash = node.hash
            assertTrue(
                "hash must be 16-char lowercase hex, was $hash",
                hash != null && hash.matches(Regex("[0-9a-f]{16}"))
            )
        }
    }

    @Test
    fun hashStableAcrossIdenticalExtractionsAndRevBumps() {
        val extractor = BridgeTreeExtractor()
        val first = extractor.extractTree(activity)
        val second = extractor.extractTree(activity)
        assertEquals(hashOf(first, "Click Me"), hashOf(second, "Click Me"))

        extractor.bumpRev()
        val afterBump = extractor.extractTree(activity)
        assertNotEquals(first.rev, afterBump.rev)
        assertEquals(hashOf(first, "Click Me"), hashOf(afterBump, "Click Me"))
    }

    @Test
    fun hashStableAcrossNonStructuralTextTweakElsewhere() {
        val extractor = BridgeTreeExtractor()
        val before = extractor.extractTree(activity)
        val inputHashBefore = hashOf(before, "hello")

        // Non-structural update: tweak the OTHER node's text.
        testButton.text = "Click Me Renamed"
        org.robolectric.shadows.ShadowLooper.idleMainLooper()
        val after = extractor.extractTree(activity)

        assertEquals(inputHashBefore, hashOf(after, "hello"))
    }

    @Test
    fun hashChangesOnOwnTextChange() {
        val extractor = BridgeTreeExtractor()
        val before = extractor.extractTree(activity)
        val hashBefore = hashOf(before, "Click Me")

        testButton.text = "Click Me Renamed"
        org.robolectric.shadows.ShadowLooper.idleMainLooper()
        val after = extractor.extractTree(activity)

        val hashAfter = hashOf(after, "Click Me Renamed")
        assertNotEquals(hashBefore, hashAfter)
    }

    @Test
    fun hashIdenticalForSubtreeQuery() {
        // Tag must be set before BOTH extractions or the ancestor segment changes.
        layout.tag = "my_container"
        org.robolectric.shadows.ShadowLooper.idleMainLooper()

        val extractor = BridgeTreeExtractor()
        val full = extractor.extractTree(activity)
        val fullHash = hashOf(full, "Click Me")

        // Subtree query returns the same node with a different traversal root;
        // its hash must still equal the full-tree hash so /tap by hash resolves.
        val subtree = extractor.extractTree(activity, subtreeId = "my_container")
        assertEquals(fullHash, hashOf(subtree, "Click Me"))
    }

    @Test
    fun hashChangesWhenAncestryChanges() {
        val extractor = BridgeTreeExtractor()
        val before = extractor.extractTree(activity)
        val hashBefore = hashOf(before, "hello")

        // Re-parent the input under a differently-identified container.
        layout.removeView(testInput)
        val otherContainer = LinearLayout(activity).apply {
            id = 201
            tag = "other_container"
        }
        layout.addView(otherContainer)
        otherContainer.addView(testInput)
        layout.measure(
            android.view.View.MeasureSpec.makeMeasureSpec(1080, android.view.View.MeasureSpec.EXACTLY),
            android.view.View.MeasureSpec.makeMeasureSpec(1920, android.view.View.MeasureSpec.EXACTLY)
        )
        layout.layout(0, 0, 1080, 1920)
        org.robolectric.shadows.ShadowLooper.idleMainLooper()

        val after = extractor.extractTree(activity)
        val hashAfter = hashOf(after, "hello")
        assertNotEquals(hashBefore, hashAfter)
    }
}
