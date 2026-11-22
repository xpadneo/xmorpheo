/* SPDX-License-Identifier: GPL-2.0-only */
/*
 * Map layout and integer math of the xmorpheo HID-BPF program.
 *
 * This header is shared by the BPF program and a host build that tests it
 * against the Rust reference in xmorpheo/src/curve.rs and plan.rs. The
 * includer provides the __u32 style types and these macros, each of which
 * returns a pointer or NULL:
 *
 *   XM_FIELD(i)        field table entry i, a struct xm_field
 *   XM_OUTPUT(i)       output table entry i, a struct xm_output
 *   XM_TERM(i)         term table entry i, a struct xm_term
 *   XM_CURVE(i)        curve i, a struct xm_curve
 *   XM_KNOT(i)         knot i of the knot pool, a struct xm_knot
 *   XM_WINDOW(f, n)    the n bytes of the report that hold field f
 *
 * and the counts XM_NR_FIELDS and XM_NR_OUTPUTS.
 */

#ifndef XMORPHEO_H
#define XMORPHEO_H

/* Most fields per device, read or written. A power of two. */
#define XM_MAX_FIELDS 16
/* Most outputs per device. */
#define XM_MAX_OUTPUTS 16
/* Most terms per device, a power of two, and per output. */
#define XM_MAX_TERMS 32
#define XM_OUTPUT_TERMS 4
/* Most curves on the system. */
#define XM_MAX_CURVES 64
/* Size of the knot pool shared by all curves. */
#define XM_MAX_KNOTS 8192
/* Most knots per curve, and the binary search steps over them. */
#define XM_CURVE_KNOTS 1024
#define XM_SEARCH_STEPS 10
/* Largest field, in bits. */
#define XM_MAX_BITS 16

/* Fraction bits of the fixed point format, and 1.0 in it. */
#define XM_FRAC 24
#define XM_ONE (1LL << XM_FRAC)

/* A field of the report, read and possibly written. */
struct xm_field {
	/* Logical range. */
	__s32 min;
	__s32 max;
	/* Bit offset in the report, including the report ID byte. */
	__u32 bit_offset;
	/* Expected length of the input report, in bytes. */
	__u16 report_len;
	/* Report ID, or 0 if the device uses none. */
	__u8 report_id;
	__u8 bit_size;
	__u8 is_signed;
	__u8 pad[3];
};

/* A field to write: the sum of its terms, denormalized. */
struct xm_output {
	/* Multiplier from Q24 back to the raw range of the field. */
	__u64 out_mul;
	/* Index into the field table. */
	__u16 field;
	/* First entry in the term table, and the number of terms. */
	__u16 first_term;
	__u8 nr_terms;
	/* 1 for centered axes (-1..1), 0 for unipolar axes (0..1). */
	__u8 centered;
	__u16 pad;
};

/* One term of an output: a curve over a source field. */
struct xm_term {
	/* Multiplier from the raw range of the source to Q24. */
	__u64 in_mul;
	/* Index into the field table. */
	__u16 field;
	/* Index into the curves map. */
	__u16 curve;
	/* How the source is normalized, like xm_output.centered. */
	__u8 centered;
	__u8 pad[3];
};

/* A slice of the knot pool. */
struct xm_curve {
	__u32 first;
	__u32 count;
};

/* A support point in Q24, with the slope to the next knot. */
struct xm_knot {
	__s32 x;
	__s32 y;
	__s64 slope;
};

static __always_inline __s64 xm_lowest(__u8 centered)
{
	return centered ? -XM_ONE : 0;
}

/* Normalizes a raw value of a logical range to Q24. */
static __always_inline __s64 xm_normalize(__u64 in_mul, __s32 min, __u8 centered, __s32 raw)
{
	__u64 off = (__u64)((__s64)raw - (__s64)min);

	return (__s64)((off * in_mul + (1ULL << 15)) >> 16) + xm_lowest(centered);
}

/* Converts a Q24 value back to a raw value, clamped to the range. */
static __always_inline __s32 xm_denormalize(__u64 out_mul, __s32 min, __u8 centered, __s64 y)
{
	__s64 lowest = xm_lowest(centered);
	__u64 u;

	if (y < lowest)
		y = lowest;
	if (y > XM_ONE)
		y = XM_ONE;
	u = (__u64)(y - lowest);
	return (__s32)((__s64)min + (__s64)((u * out_mul + (1ULL << 31)) >> 32));
}

/* Interpolates within the segment that starts at knot k. */
static __always_inline __s64 xm_segment(const struct xm_knot *k, __s64 x)
{
	return (__s64)k->y + (((x - (__s64)k->x) * k->slope + (1LL << (XM_FRAC - 1))) >> XM_FRAC);
}

/* Bytes of the report that hold a field, at most three for 16 bits. */
static __always_inline __u32 xm_field_bytes(const struct xm_field *f)
{
	__u32 end = (f->bit_offset & 7) + f->bit_size;

	return end > 16 ? 3 : end > 8 ? 2 : 1;
}

/* Extracts a field from the bytes that hold it. */
static __always_inline __s32 xm_field_get(const struct xm_field *f, const __u8 *d,
					  __u32 bytes)
{
	__u32 v = d[0], raw, mask = (1U << f->bit_size) - 1;

	if (bytes > 1)
		v |= (__u32)d[1] << 8;
	if (bytes > 2)
		v |= (__u32)d[2] << 16;
	raw = (v >> (f->bit_offset & 7)) & mask;
	if (f->is_signed && (raw & (1U << (f->bit_size - 1))))
		return (__s32)(raw | ~mask);
	return (__s32)raw;
}

/* Inserts a field into the bytes that hold it, keeping the other bits. */
static __always_inline void xm_field_put(const struct xm_field *f, __u8 *d, __u32 bytes,
					 __s32 value)
{
	__u32 shift = f->bit_offset & 7, mask = (1U << f->bit_size) - 1;
	__u32 v = d[0];

	if (bytes > 1)
		v |= (__u32)d[1] << 8;
	if (bytes > 2)
		v |= (__u32)d[2] << 16;
	v = (v & ~(mask << shift)) | (((__u32)value & mask) << shift);
	d[0] = v;
	if (bytes > 1)
		d[1] = v >> 8;
	if (bytes > 2)
		d[2] = v >> 16;
}

/*
 * Evaluates a curve at a Q24 input. Returns 0 on success, or -1 for curves
 * that are empty or out of bounds.
 */
static __always_inline int xm_curve_eval(const struct xm_curve *c, __s64 x, __s64 *y)
{
	const struct xm_knot *k;
	__u32 lo = 0, hi, mid, i;

	if (c->count == 0 || c->count > XM_CURVE_KNOTS)
		return -1;
	hi = c->count - 1;
	for (i = 0; i < XM_SEARCH_STEPS; i++) {
		if (lo >= hi)
			break;
		mid = (lo + hi + 1) / 2;
		k = XM_KNOT(c->first + mid);
		if (!k)
			return -1;
		if ((__s64)k->x <= x)
			lo = mid;
		else
			hi = mid - 1;
	}
	k = XM_KNOT(c->first + lo);
	if (!k)
		return -1;
	*y = xm_segment(k, x);
	return 0;
}

static __always_inline int xm_in_range(const struct xm_field *f, __s32 raw)
{
	return raw >= f->min && raw <= f->max;
}

/*
 * Rewrites a report in three phases, like a transaction: read all fields,
 * compute all outputs from the values the device sent, then write the
 * outputs that changed. So no output depends on the order of the fields or
 * on other outputs. An output keeps its value if its field or any source of
 * its terms is outside its logical range.
 */
static __always_inline void xm_rewrite(void *ctx, __u32 size, __u8 report_id)
{
	__s32 in[XM_MAX_FIELDS], out[XM_MAX_FIELDS];
	__u8 valid[XM_MAX_FIELDS], changed[XM_MAX_FIELDS];
	const struct xm_output *o;
	const struct xm_curve *c;
	const struct xm_field *f;
	const struct xm_term *t;
	__u32 i, j, bytes, ti, fi;
	__s64 sum, y;
	__u8 *d;
	int ok;

	(void)ctx; /* only used by XM_WINDOW in the BPF program */
	for (i = 0; i < XM_MAX_FIELDS; i++) {
		/* The verifier wants the whole buffers initialized. */
		in[i] = 0;
		out[i] = 0;
		valid[i] = 0;
		changed[i] = 0;
		if (i >= XM_NR_FIELDS)
			continue;
		f = XM_FIELD(i);
		if (!f || f->bit_size == 0 || f->bit_size > XM_MAX_BITS)
			continue;
		/*
		 * Leave reports alone that do not match the descriptor. Longer
		 * reports are fine: devices may pad them to the size of their
		 * endpoint, and the HID core ignores the extra bytes too.
		 */
		if (size < f->report_len || (f->report_id && f->report_id != report_id))
			continue;
		bytes = xm_field_bytes(f);
		d = XM_WINDOW(f, bytes);
		if (!d)
			continue;
		in[i] = xm_field_get(f, d, bytes);
		valid[i] = 1;
	}

	for (i = 0; i < XM_MAX_OUTPUTS; i++) {
		if (i >= XM_NR_OUTPUTS)
			break;
		o = XM_OUTPUT(i);
		if (!o)
			continue;
		fi = o->field & (XM_MAX_FIELDS - 1);
		f = XM_FIELD(fi);
		if (!f || !valid[fi] || !xm_in_range(f, in[fi]))
			continue;
		sum = 0;
		ok = o->nr_terms > 0;
		for (j = 0; j < XM_OUTPUT_TERMS; j++) {
			if (j >= o->nr_terms)
				break;
			ti = (o->first_term + j) & (XM_MAX_TERMS - 1);
			t = XM_TERM(ti);
			if (!t) {
				ok = 0;
				break;
			}
			fi = t->field & (XM_MAX_FIELDS - 1);
			f = XM_FIELD(fi);
			c = XM_CURVE(t->curve);
			if (!f || !c || !valid[fi] || !xm_in_range(f, in[fi])) {
				ok = 0;
				break;
			}
			if (xm_curve_eval(c, xm_normalize(t->in_mul, f->min, t->centered, in[fi]),
					  &y)) {
				ok = 0;
				break;
			}
			sum += y;
		}
		if (!ok)
			continue;
		fi = o->field & (XM_MAX_FIELDS - 1);
		f = XM_FIELD(fi);
		if (!f)
			continue;
		out[fi] = xm_denormalize(o->out_mul, f->min, o->centered, sum);
		changed[fi] = 1;
	}

	for (i = 0; i < XM_MAX_FIELDS; i++) {
		if (!changed[i] || out[i] == in[i])
			continue;
		f = XM_FIELD(i);
		if (!f)
			continue;
		bytes = xm_field_bytes(f);
		d = XM_WINDOW(f, bytes);
		if (d)
			xm_field_put(f, d, bytes, out[i]);
	}
}

#endif /* XMORPHEO_H */
