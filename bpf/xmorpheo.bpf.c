// SPDX-License-Identifier: GPL-2.0-only
/*
 * xmorpheo: response curves for HID axes.
 *
 * Rewrites selected axis fields of each input report as a sum of curves
 * over fields of the same report. The program knows nothing about devices
 * or formulas: the loader fills the field, output, and term tables of each
 * instance, and the curve and knot maps, which instances for other devices
 * can share.
 */

#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct xm_field;
static __always_inline __u8 *window(struct hid_bpf_ctx *hctx, const struct xm_field *f,
				    __u32 bytes);
static __always_inline const void *lookup(void *map, __u32 index);

#define XM_FIELD(i) ((const struct xm_field *)lookup(&fields, (i)))
#define XM_OUTPUT(i) ((const struct xm_output *)lookup(&outputs, (i)))
#define XM_TERM(i) ((const struct xm_term *)lookup(&terms, (i)))
#define XM_CURVE(i) ((const struct xm_curve *)lookup(&curves, (i)))
#define XM_KNOT(i) ((const struct xm_knot *)lookup(&knots, (i)))
#define XM_WINDOW(f, n) window(ctx, (f), (n))
#define XM_NR_FIELDS nr_fields
#define XM_NR_OUTPUTS nr_outputs

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, 16);
	__type(key, __u32);
	__type(value, struct xm_field);
} fields SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, 16);
	__type(key, __u32);
	__type(value, struct xm_output);
} outputs SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, 32);
	__type(key, __u32);
	__type(value, struct xm_term);
} terms SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, 64);
	__type(key, __u32);
	__type(value, struct xm_curve);
} curves SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, 8192);
	__type(key, __u32);
	__type(value, struct xm_knot);
} knots SEC(".maps");

/* Set by the loader before loading. */
const volatile __u32 nr_fields = 0;
const volatile __u32 nr_outputs = 0;

#include "xmorpheo.h"

_Static_assert(sizeof(struct xm_field) == 20, "struct xm_field layout");
_Static_assert(sizeof(struct xm_output) == 16, "struct xm_output layout");
_Static_assert(sizeof(struct xm_term) == 16, "struct xm_term layout");
_Static_assert(sizeof(struct xm_curve) == 8, "struct xm_curve layout");
_Static_assert(sizeof(struct xm_knot) == 16, "struct xm_knot layout");

static __always_inline const void *lookup(void *map, __u32 index)
{
	return bpf_map_lookup_elem(map, &index);
}

/*
 * Returns the bytes of the report that hold a field of up to 16 bits, or
 * NULL. The window size must be a constant for the kfunc.
 */
static __always_inline __u8 *window(struct hid_bpf_ctx *hctx, const struct xm_field *f,
				    __u32 bytes)
{
	__u32 byte = f->bit_offset >> 3;

	if (byte + bytes > f->report_len)
		return NULL;
	if (bytes == 3)
		return hid_bpf_get_data(hctx, byte, 3);
	if (bytes == 2)
		return hid_bpf_get_data(hctx, byte, 2);
	return hid_bpf_get_data(hctx, byte, 1);
}

SEC("struct_ops/hid_device_event")
int BPF_PROG(xmorpheo_event, struct hid_bpf_ctx *hctx, enum hid_report_type type, __u64 source)
{
	__u32 size = hctx->size;
	__u8 *d;

	if (type != HID_INPUT_REPORT || size == 0)
		return 0;
	d = hid_bpf_get_data(hctx, 0, 1);
	if (!d)
		return 0;
	xm_rewrite(hctx, size, d[0]);
	return 0;
}

SEC(".struct_ops.link")
struct hid_bpf_ops xmorpheo = {
	.hid_device_event = (void *)xmorpheo_event,
};

char _license[] SEC("license") = "GPL";
