# Star ratings

PicOrg uses the standard XMP rating scale — an integer from `0` to
`5` — stored as `xmp:Rating` inside the photo's XMP.

## Setting a rating

The **Rating** row in the details panel shows five stars. Click the
star at the level you want. Clicking star 3, for example, sets the
rating to 3 and highlights stars 1–3.

- **Click the same star again** to clear the rating back to
  "not rated".
- Rating changes save immediately, both to the sidecar and into the
  source file's embedded XMP.

## Filtering by rating

The **Rating** section of the sidebar lists five thresholds:

- **5 stars** — show only 5-star photos.
- **4 stars and up** — 4 and 5.
- **3 stars and up** — 3, 4, and 5.
- **2 stars and up** — 2, 3, 4, and 5.
- **1 star and up** — everything with any rating (excludes unrated).

Click a threshold to apply it as a filter. Click **All photos** at the
top of the sidebar (or the same threshold again) to clear.

## Batch rating

Select multiple photos, then click a star in the **Set rating**
control in the details panel. Every selected photo gets that rating
in one shot. This is the fastest way to sweep through a shoot and
promote your keepers.

## Interoperability

`xmp:Rating` is the industry-standard XMP rating field:

- **Adobe Lightroom** reads and writes the same field. Import a
  PicOrg-rated folder into Lightroom and every rating is there.
- **Adobe Bridge** shows the ratings.
- **Windows Photos app** uses the same field for its star rating.
- **digiKam, XnView, FastStone, IrfanView** all recognise it.

If a photo already had a rating in another tool, PicOrg reads it on
first scan; when you edit it in PicOrg, the change flows back through
the sidecar and embedded XMP to the other tool the next time it
refreshes its index.
