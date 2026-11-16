// SPDX-License-Identifier: GPL-2.0-only
/*
 * Host build of the BPF-side math, for tests against the Rust reference.
 * It is not part of the BPF program.
 */

#include <stddef.h>
#include <linux/types.h>

#ifndef __always_inline
#define __always_inline inline __attribute__((always_inline))
#endif

struct xm_field;
struct xm_output;
struct xm_term;
struct xm_curve;
struct xm_knot;

/* The tables of one rewrite, like the maps of the BPF program. */
struct host_tables {
	const struct xm_field *fields;
	__u32 nr_fields;
	const struct xm_output *outputs;
	__u32 nr_outputs;
	const struct xm_term *terms;
	__u32 nr_terms;
	const struct xm_curve *curves;
	__u32 nr_curves;
	const struct xm_knot *knots;
	__u32 nr_knots;
	__u8 *report;
	__u32 report_len;
};

static _Thread_local const struct host_tables *host;

#define HOST_AT(table, count, i) ((__u32)(i) < host->count ? &host->table[i] : NULL)
#define XM_FIELD(i) HOST_AT(fields, nr_fields, i)
#define XM_OUTPUT(i) HOST_AT(outputs, nr_outputs, i)
#define XM_TERM(i) HOST_AT(terms, nr_terms, i)
#define XM_CURVE(i) HOST_AT(curves, nr_curves, i)
#define XM_KNOT(i) HOST_AT(knots, nr_knots, i)
#define XM_WINDOW(f, n) host_window((f), (n))
#define XM_NR_FIELDS host->nr_fields
#define XM_NR_OUTPUTS host->nr_outputs

static __u8 *host_window(const struct xm_field *f, __u32 bytes);

#include "xmorpheo.h"

static __u8 *host_window(const struct xm_field *f, __u32 bytes)
{
	__u32 byte = f->bit_offset >> 3;

	if (byte + bytes > f->report_len || byte + bytes > host->report_len)
		return NULL;
	return host->report + byte;
}

/* Rewrites a report like the BPF program, with the given tables. */
void xm_host_rewrite(const struct host_tables *tables)
{
	host = tables;
	if (tables->report_len > 0)
		xm_rewrite(NULL, tables->report_len, tables->report[0]);
	host = NULL;
}

__s32 xm_host_field_get(const struct xm_field *f, const __u8 *report)
{
	return xm_field_get(f, report + (f->bit_offset >> 3), xm_field_bytes(f));
}

void xm_host_field_put(const struct xm_field *f, __u8 *report, __s32 value)
{
	xm_field_put(f, report + (f->bit_offset >> 3), xm_field_bytes(f), value);
}

/* Sizes and member offsets, to compare with the skeleton types. */
const size_t xm_host_layout[] = {
	sizeof(struct xm_field),
	offsetof(struct xm_field, min),
	offsetof(struct xm_field, max),
	offsetof(struct xm_field, bit_offset),
	offsetof(struct xm_field, report_len),
	offsetof(struct xm_field, report_id),
	offsetof(struct xm_field, bit_size),
	offsetof(struct xm_field, is_signed),
	sizeof(struct xm_output),
	offsetof(struct xm_output, out_mul),
	offsetof(struct xm_output, field),
	offsetof(struct xm_output, first_term),
	offsetof(struct xm_output, nr_terms),
	offsetof(struct xm_output, centered),
	sizeof(struct xm_term),
	offsetof(struct xm_term, in_mul),
	offsetof(struct xm_term, field),
	offsetof(struct xm_term, curve),
	offsetof(struct xm_term, centered),
	sizeof(struct xm_curve),
	offsetof(struct xm_curve, first),
	offsetof(struct xm_curve, count),
	sizeof(struct xm_knot),
	offsetof(struct xm_knot, x),
	offsetof(struct xm_knot, y),
	offsetof(struct xm_knot, slope),
};

/* Limits, to compare with the Rust side. */
const __u32 xm_host_limits[] = {
	XM_MAX_FIELDS, XM_MAX_OUTPUTS, XM_MAX_TERMS, XM_OUTPUT_TERMS,
	XM_MAX_CURVES, XM_MAX_KNOTS, XM_CURVE_KNOTS, XM_SEARCH_STEPS,
	XM_MAX_BITS, XM_FRAC,
};
