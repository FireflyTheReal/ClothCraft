# Editing and inspecting a garment

## Sew two boundaries

Choose **Sew**, click the first boundary, then the second in either viewport.
The card beside the pointer names both sides and shows their lengths. Click
**Sew these edges**, press Enter, or click empty canvas to confirm. Unequal
lengths instead offer **Sew with easing**: this explicitly accepts the mismatch.
Easing changes the seam correspondence, not the fabric's elasticity.

Use **Flip / F** to reverse side B, or Shift + wheel over the canvas. Partial
ranges and ordered multi-edge chains remain available. Successful confirmation
clears the draft so the next click starts a new seam. Existing seams can be
selected and deleted; Undo restores them.

## Inspect only the rendered mesh

Open the **3D viewport gear → Clean view**. This hides seam samples, seam links,
boundary highlights, range handles, selection markers, issue marks, gizmos,
wireframe and strain colors. It does not remove seams or alter simulation.
Switch it off to restore the previous overlay settings. The background and
floor retain their independent display controls.

## Grab a larger area

The **Grab** tool shows **Radius (px)** and **Strength** above the 3D view.
Radius is measured on screen and converted to a surface distance at the initial
pick. The selected attachments remain fixed until release. The geodesic brush
stays on the picked panel's connected triangle component, so a nearby folded
layer or separate panel is not captured solely because it is close in space.
It does not cross seams. The visible circle indicates the screen radius;
surface connectivity and folds can make the actual patch smaller.

Each attached vertex retains its initial offset from the pointer. The brush
therefore translates a patch instead of pulling every vertex to one point.
Strength multiplies a total 150 N/m spring budget at 1×. A nonempty brush
assigns 25% to the barycentric point and distributes 75% over the patch by
mass and radial falloff. Enlarging the brush does not multiply total strength
by vertex count. Radius zero exactly retains the old point-grab model. Settings apply on the next drag.

These are finite implicit springs in both CPU and hybrid GPU solvers. They use
the existing collision restrictions and do not teleport vertices, modify the
rest shape, or disable contacts. Stronger grabs may need more iterations and
can cause visible stretch. Releasing removes every temporary attachment.

## Select and move points across pieces

In **Edit**, choose **Points** and **Box** or **Lasso**. Drag a selection across
several patterns. Shift adds points; Shift-click toggles individual anchors.
Drag a selected point to move the group, or use arrow keys for 1 mm increments
(Shift: 10 mm). Each pattern's board rotation is accounted for. The edit is one
undoable transaction and still validates contours and seams.

Editing one linked-mirror member updates its partner. Editing both members at
once is rejected rather than choosing a conflicting source implicitly.
This is 2D contour editing; 3D marquee selection and group placement gizmos
remain open roadmap tasks.

Select panel bodies and press **Delete** to remove their group. Text fields,
selected contour points and selected seams retain their own Delete behavior.
At least one panel must remain. Removing a panel also removes its seam and
mirror references; Undo restores the whole edit.

[Validation and numerical limits](editor-release-validation.md).
