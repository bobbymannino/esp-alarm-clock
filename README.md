# ESP Alarm Clock

A simple alarm clock written in Rust using an ESP32.

## Useful Commands

```sh
# Chip revision, flash size, CPU speed
espflash board-info
```

```sh
# Read the partition table as bytes
# 0x8000 is the start address of the partition table
# 0xc00 is the size of the partition table
espflash read-flash 0x8000 0xc00 pt.bin

# This decodes the pt.bin into human readable format
espflash partition-table pt.bin
```

```sh
# Dump the raw NVS (storage)
# espflash read-flash <address> <size> <file>
espflash read-flash 0x9000 0x6000 nvs.bin

.embuild/espressif/esp-idf/v5.5.4/components/nvs_flash/nvs_partition_tool/nvs_tool.py [-d written] nvs.bin
```
