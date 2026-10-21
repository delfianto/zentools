// SPDX-License-Identifier: GPL-2.0-only
/* One-shot read-only research bridge for MSI MS-7E49 BIOS 1.AA3.
 * The inspected WMAA methods 4/6 only copy WCNS to a response buffer.
 * No user-selected method, object ID, SMI, physical-memory read or setter.
 */
#include <linux/acpi.h>
#include <linux/dmi.h>
#include <linux/module.h>
#include <linux/proc_fs.h>
#include <linux/seq_file.h>
#include <linux/slab.h>
#include <linux/unaligned.h>

#include "objects.h"

#define AOD_GUID "95F20B61-3D44-41F8-87E1-03CBF94C4E8F"

struct result {
	acpi_status status;
	u32 length;
	u32 words[5];
};

static struct result results[ARRAY_SIZE(queries)];
static struct proc_dir_entry *probe_entry;

static int values_show(struct seq_file *seq, void *unused)
{
	size_t i;

	for (i = 0; i < ARRAY_SIZE(queries); i++) {
		const struct result *r = &results[i];

		seq_printf(seq, "%08x %u %08x %u %08x %08x %08x %08x %08x\n", queries[i].id,
			   queries[i].method, r->status, r->length, r->words[0], r->words[1],
			   r->words[2], r->words[3], r->words[4]);
	}
	return 0;
}

static int __init probe_init(void)
{
	size_t i, j;

	if (!dmi_match(DMI_BOARD_NAME, "MPG X870E CARBON WIFI (MS-7E49)") ||
	    !dmi_match(DMI_BIOS_VERSION, "1.AA3"))
		return -ENODEV;
	if (!wmi_has_guid(AOD_GUID))
		return -ENODEV;
	for (i = 0; i < ARRAY_SIZE(queries); i++) {
		__le32 id = cpu_to_le32(queries[i].id);
		struct acpi_buffer in = { sizeof(id), &id };
		struct acpi_buffer out = { ACPI_ALLOCATE_BUFFER, NULL };
		union acpi_object *obj;
		struct result *r = &results[i];

		if (queries[i].method != 4 && queries[i].method != 6)
			return -EINVAL;
		r->status = wmi_evaluate_method(AOD_GUID, 0, queries[i].method, &in, &out);
		obj = out.pointer;
		if (ACPI_SUCCESS(r->status) && obj && obj->type == ACPI_TYPE_BUFFER) {
			r->length = obj->buffer.length;
			for (j = 0; j < ARRAY_SIZE(r->words) && (j + 1) * 4 <= r->length; j++)
				r->words[j] = get_unaligned_le32(obj->buffer.pointer + j * 4);
		}
		kfree(out.pointer);
	}
	probe_entry = proc_create_single("zentools_aod_probe", 0444, NULL, values_show);
	return probe_entry ? 0 : -ENOMEM;
}

static void __exit probe_exit(void)
{
	proc_remove(probe_entry);
}
module_init(probe_init);
module_exit(probe_exit);
MODULE_LICENSE("GPL");
MODULE_DESCRIPTION("Bounded read-only AMD AOD getter research probe");
