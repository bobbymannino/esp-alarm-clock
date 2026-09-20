# ESP Alarm Clock

A simple alarm clock written in Rust using an ESP32.

## Usage

```sh
# Flash the ESP with the firmware
cargo run

# Run the GUI interface for reading/settings alarms
cargo g

# Run the unit tests
cargo t
```

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

# Shows a simple hex/ascii dump of the NVS
hexdump -C nvs.bin

# Pipe to less for quick search
hexdump -C nvs.bin | less

# Print all readable strings in the NVS
strings nvs.bin

# Shows a more detailed view of the NVS
.embuild/espressif/esp-idf/v5.5.4/components/nvs_flash/nvs_partition_tool/nvs_tool.py [-d written] nvs.bin
```

## Flash Sizing

https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/storage/nvs_flash.html

The NVS flash size is 4096 bytes. This is split into pages. Each page looks like
this:

```
+-----------+--------------+-------------+-------------------------+
| State (4) | Seq. no. (4) | version (1) | Unused (19) | CRC32 (4) |   Header (32)
+-----------+--------------+-------------+-------------------------+
|                Entry state bitmap (32)                           |
+------------------------------------------------------------------+
|                       Entry 0 (32)                               |
+------------------------------------------------------------------+
|                       Entry n (32)                               |
+------------------------------------------------------------------+
```

Each entry looks like this:

```
+--------+----------+----------+----------------+-----------+---------------+----------+
| NS (1) | Type (1) | Span (1) | ChunkIndex (1) | CRC32 (4) |    Key (16)   | Data (8) |
+--------+----------+----------+----------------+-----------+---------------+----------+

                                         Primitive  +--------------------------------+
                                        +-------->  |     Data (8)                   |
                                        | Types     +--------------------------------+
                   +-> Fixed length --
                   |                    |           +---------+--------------+---------------+-------+
                   |                    +-------->  | Size(4) | ChunkCount(1)| ChunkStart(1) | Rsv(2)|
    Data format ---+                    Blob Index  +---------+--------------+---------------+-------+
                   |
                   |                             +----------+---------+-----------+
                   +->   Variable length   -->   | Size (2) | Rsv (2) | CRC32 (4) |
                        (Strings, Blob Data)     +----------+---------+-----------+
```
