# GeoGebra on the bench chain (PROVISIONAL)

> **PROVISIONAL, not a baseline.** These numbers were taken while the machine
> was under heavy unrelated load (load average 17 to 33 on 10 cores). Repeat
> them on an idle machine before comparing anything against them.

## What was measured

`tools/ggb-harness` (`harness.sh chain <steps>`) builds in headless GeoGebra
the chain from `bench()` in `crates/geo-cli/src/main.rs`: free points
`A = (0.3, 0.7)` and `B = (10, 2)`, then per step a line through `A` and the
previous point, the circle around `A` through it, the perpendicular through
`A`, its intersection with the circle (index 1), the midpoint, the line through
`A` and the midpoint, and that line's intersection with the circle (index 2),
which becomes the next previous point. Every chain object depends on `A`.
Three unrelated objects (`Z_1`, `Z_2` and their midpoint) are added at the end.

Each drag is `GeoPoint.setCoords` plus `updateCascade()` on `A`, a full update
of everything that depends on it. The views are detached
(`Kernel.setNotifyViewsActive(false)`), so only the construction kernel is
timed, not the headless graphics view. After the build, one warm-up round of
200 drags runs along the same path as the Rust bench
(`A = (0.3 + cos t, 0.7 + sin t)`, `t = 0.05 i`), then three timed rounds of
200 drags. The harness checks that no chain object is undefined after the
build, the warm-up and the timed rounds.

- GeoGebra commit `112919f2d450ec7430a5cd3b65dba226fdc43b74`, Classic configuration
- JVM: Azul Zulu OpenJDK 64-Bit Server VM 17.0.12+7-LTS, flags
  `-Xms4g -Xmx4g -XX:+UseG1GC`
- macOS on arm64, 10 cores

## The full chain does not fit

The bench size (15,000 steps, 105,002 chain objects) cannot be built in
GeoGebra. Every GeoGebra object keeps a set of all algorithms that depend on
it, directly or not (`GeoElement.addToUpdateSets` walks up the graph for each
new algorithm). In a chain, where every object depends on all earlier ones,
memory and build time grow with the square of the chain length:

| Steps | Chain objects | Build | Heap after build | Update set of `A` |
| ---: | ---: | ---: | ---: | ---: |
| 250 | 1,752 | 0.8 s | 179 MB | 2,250 algorithms |
| 500 | 3,502 | 2.8 s | 673 MB | 4,500 algorithms |
| 1,000 | 7,002 | 14.7 s | 2,638 MB | 9,000 algorithms |

Doubling the steps multiplies the heap by about 3.9. Extrapolating to 15,000
steps gives roughly 600 GB of heap, so the measurement stops at 1,000 steps.
(Each intersection command creates two algorithms, which is why `A` has 9
algorithms per step for 7 objects.)

## Drag times

| Steps | Dependent objects | Per drag of `A` (3 rounds) | Best | Per dependent object |
| ---: | ---: | --- | ---: | ---: |
| 250 | 1,750 | 0.123, 0.128, 0.125 ms | 0.123 ms | 70 ns |
| 500 | 3,500 | 0.351, 0.378, 0.361 ms | 0.351 ms | 100 ns |
| 1,000 | 7,000 | 1.212, 1.210, 1.132 ms | 1.132 ms | 162 ns |

Dragging `Z_1`, on which only one midpoint depends, took about 90 ns per drag
at every size.

The cost per object rises with the size of the construction, probably because
the working set outgrows the caches; a linear extrapolation from 1,000 steps
would give at least 17 ms per drag at the bench size (105,000 objects at
162 ns), but GeoGebra cannot reach that size at all.

The Rust bench chooses intersection points by its own rule (`which` 0 and 1)
while GeoGebra uses indices 1 and 2. The two chains therefore need not pass
through the same points; both stay defined, and the amount of work per drag is
the same.

## geo-kernel on the same chain (PROVISIONAL)

`cargo run --release -p geo-cli -- bench <steps>` on the same machine, also
under unrelated load (load average about 7), so equally provisional:

| Steps | Dependent objects | Per drag of `A` | Per dependent object |
| ---: | ---: | ---: | ---: |
| 250 | 1,750 | 0.050 ms | 29 ns |
| 500 | 3,500 | 0.110 ms | 31 ns |
| 1,000 | 7,000 | 0.241 ms | 34 ns |
| 15,000 | 105,000 | 4.721 ms | 45 ns |

The two sides were not measured under the same load, so the ratio (roughly
2.5 to 4.7 times at these sizes) is an indication only.
