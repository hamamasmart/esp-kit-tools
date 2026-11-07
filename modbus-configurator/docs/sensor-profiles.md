# RS485 Sensor Register Maps & Configuration Reference

How to read and update (reprogram) the **slave address** (and baud rate) of each RS485 sensor
family used by the fleet, over Modbus-RTU. This is the knowledge the configurator firmware encodes;
the per-model wire profiles live in `src/profiles.rs`.

All sensors are Modbus-RTU slaves on RS485, configured through **holding registers**:

- **Read** registers with function code `0x03` (read holding registers).
- **Write** registers with function code `0x06` (write single register) *or* `0x10` (write
  multiple registers), depending on the family — see the per-model table below.

Wiring/timing that works in practice: UART 8N1 (8 data bits, no parity, 1 stop bit), Modbus
response timeout 1000 ms, turnaround delay 5 ms. Response frames always carry a CRC16 (Modbus,
poly `0xA001` init `0xFFFF`, low byte transmitted first) — validate it before accepting a frame.

---

## Register maps per sensor family

| Family (`sensor_type`) | Address register | Baud-rate register | Write FC | Slave range | Power cycle? | Default addr/baud |
| :---                   | :---:            | :---:              | :---:    | :---:       | :---:        | :---:             |
| `SN-300AL-GH-N01` (N01 RS485 family) | `0x07D0` | `0x07D1` | `0x06` | 1–254    | No  | — |
| `LightIntensityRs485`  | `0x0100`         | `0x0101`           | `0x06`   | 1–251       | **Yes**      | — |
| `SoilMultiParameter`   | `0x0080`         | `0x0081`           | **`0x10`** | 1–247 (assumed standard limit; manual does not state) | No | **2** @ 9600 |

### Baud-rate encodings

The two older families store the baud rate as a small **integer code**:

| Register value | Baud rate |
| :---:          | :---:     |
| `0`            | 2400      |
| `1`            | 4800      |
| `2`            | 9600      |

Anything else is unsupported for those families.

The **soil multi-parameter family** stores the baud rate as **baud ÷ 10 with the decimal digits
packed as hex nibbles** (per the manual's example, where 19200 baud is written as data
`0x19 0x20` — i.e. the digits of "1920"):

| Register value | Baud rate |
| :---:          | :---:     |
| `0x0120`       | 1200      |
| `0x0240`       | 2400      |
| `0x0480`       | 4800      |
| `0x0960`       | 9600 (default) |
| `0x1920`       | 19200     |
| `0x3840`       | 38400 (max per manual) |

> ⚠️ This is *not* the plain numeric value (0x1920 = 6432 ≠ 1920). The sensor's firmware
> interprets the nibbles as decimal digits. Do not "optimize" this encoding without re-testing
> against a real sensor.

### Soil multi-parameter family — full config register map

From *Soil Multi-parameter Sensor Manual V2.2* (7-in-1/8-in-1 soil sensor: temperature, moisture,
EC, salt, N, P, K, pH). All config registers are R/W:

| Register | Content     | Values |
| :---     | :---        | :---   |
| `0x0080` | Device address | 1–247 (factory default **2**); broadcast `0x00` allowed for writes when only one slave is on the bus |
| `0x0081` | Baud rate   | 1200–38400, encoded as baud÷10 hex-digits (table above) |
| `0x0082` | Parity      | `1` = odd, `2` = even (0 = none, per standard Modbus) |
| `0x0083` | Data bits   | 8 or 9 |
| `0x0084` | Stop bits   | 1 or 2 |
| `0x00BF` | Analog 1 min range (temperature) | e.g. `-20.0 °C` stored as `0xFE70` (= −400, i.e. value ×10, two's complement) |
| `0x00C0` | Analog 1 max range (temperature) | e.g. `80.0 °C` stored as `0x0320` (= 800) |
| `0x00C1` | Analog 2 min range (moisture) | `0.0%` |
| `0x00C2` | Analog 2 max range (moisture) | `100.0%` |

The configurator only programs address (`0x0080`) and baud rate (`0x0081`); the remaining registers
are listed here for reference.

### Soil multi-parameter family — measurement registers (read-only knowledge)

Real-time data is read with FC `0x03` starting at `0x0000`; each value is one 16-bit big-endian
register. Non-7-in-1 variants expose a subset — read only the registers that apply:

| Register | Quantity   | Conversion |
| :---     | :---       | :---       |
| `0x0000` | Soil temperature | ÷10 °C; negative values are two's complement (e.g. `0xFFDD` → −3.5 °C) |
| `0x0001` | Soil moisture    | ÷10 % |
| `0x0002` | EC               | raw µS/cm |
| `0x0003` | Salt             | raw (mg/kg scale per vendor) |
| `0x0004` | Nitrogen (N)     | raw mg/kg |
| `0x0005` | Phosphorus (P)   | raw mg/kg |
| `0x0006` | Potassium (K)    | raw mg/kg |
| `0x0007` | pH               | ÷10 (e.g. `0x0046` → 7.0) |

Example from the manual — query 8 registers from address 1:

```
request:  00 03 00 00 00 08 45 DD
response: 01 03 10 FFDD 0164 0320 0190 0164 0164 0164 0046 A7 B5
           │  │  │  └ temp -3.5°C, moisture 35.6%, EC 800, salt 400,
           │  │  │     N 356, P 356, K 356, pH 7.0
           │  │  └ 16 data bytes
           │  └ FC 03
           └ slave address
```

Note: the **first pH measurement after power-up takes >5 minutes** to stabilize (galvanic-cell
method); all other channels stabilize in <5 s. Protection grade IP68; supply 3.3–24 V DC
(RS485 static <3 mA, measuring 25 mA, max 35 mA).

---

## Reading the current address and baud rate

Read **2 holding registers** starting at the family's address register (FC `0x03`) — the pair
returns `[slave_address, baud_code]`, with `baud_code` interpreted per the family's encoding:

```
request:  slave | 0x03 | addr_hi | addr_lo | 0x00 | 0x02 | CRC16
response: slave | 0x03 | 0x04 | addr_u16 | baud_code_u16 | CRC16
```

Must be done at the sensor's *current* slave address and baud rate.

## Updating the address (and baud rate)

Given **input** parameters (what the sensor is using now) and **output** parameters (the desired
settings), the sequence is:

1. **Write the address register** with the output slave id, addressing the sensor at the *input*
   slave id and *input* baud rate. Use the family's write FC (`0x06`, or `0x10` with `count = 1`
   for the soil family).
2. **Write the baud-rate register** with the output baud code, still at the input baud. If this
   write times out, the address change from step 1 took effect immediately (the SN-300 family does
   this) — retry the write addressing the *output* slave id, still at the input baud.
3. **Verify** by re-reading the two registers (as above) at the output slave id / output baud.
   - For models that require a power cycle (the light-intensity sensor), the sensor won't answer
     after reprogramming until it is restarted: power-cycle it first, then read back with
     input = the just-programmed output values to confirm.
4. On failure to write at all: check wiring/power and that the sensor is really at the assumed
   input slave id and baud rate.

### Wire examples from the soil manual (FC 0x10, count = 1)

Change address 2 → 1:

```
Master → Slave:  00 10 00 80 00 01 02 00 01 75 C0
                  │  │  │  │  │  │  │  └ value 0x0001
                  │  │  │  │  │  │  └ byte count (2)
                  │  │  │  │  │  └ register count (1)
                  │  │  └ register 0x0080
                  │  └ FC 0x10
                  └ old address (0x02; 0x00 = broadcast)
Slave → Master:  02 10 00 80 00 01 6C A0
```

Change baud 9600 → 19200:

```
Master → Slave:  00 10 00 81 00 01 02 19 20 BF 99
                                          └ data 0x1920 = "1920" = 19200/10
```

**Broadcast note (manual):** if the sensor's address is unknown, address `0x00` may be used for
configuration writes — but only when **a single slave is connected to the bus**. When writing via
broadcast, the response comes back under the sensor's *actual* address (see the manual's examples,
where a write addressed to `0x00` is echoed from `0x02`), which the current driver rejects as an
address mismatch. To configure a sensor of unknown address with this firmware, connect it alone on
the bus and set the input slave id to its actual address; `0x00`-broadcast support would require a
driver change (accept any response address for broadcast writes, or ignore the response and
verify by polling).
