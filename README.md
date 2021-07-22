# c2g

Turn your chess PGNs to GIFs!

## Usage

### Basic

Pass a size in pixels and an output file to `c2g`:

```shell
cat example/example.pgn | ./c2g --size 640 --output example/chess.gif
```

Will output the following 640x640 GIF:

![Example](/example/chess.gif)

### Bullet

Bullet games are good candidates for real time delay using the `--delay="real"` flag:

```shell
cat example/example_bullet.pgn | ./c2g --size 640 --delay="real" --output example/chess_bullet.gif
```

![Example-Bullet](/example/chess_bullet.gif)

### Clean

If you prefer a more cleaner look without any [features](#Features), you can disable them:

```shell
cat example/example_no_clock.pgn | ./c2g --size 640 --no-player-bars --no-terminations --output example/chess_clean.gif
```

![Example-Clean](/example/chess_clean.gif)

## Instalation

To install c2g you can download one of the binaries available from [Releases](https://github.com/tomasfarias/c2g/releases). These binaries are compiled with the default features `include-svgs` and `include-fonts`, which means that the svgs and fonts avaible at [`svgs/`](svgs/) and [`fonts/`](fonts/) respectively come bundled with the binary, which makes it so it can be run without any extra dependencies from anywhere.

## Compile from source

c2g may also be compiled from source by fetching the repository and building with `cargo`:

```shell
git clone https://github.com/tomasfarias/c2g.git
cd c2g
cargo build --release
```

Building with the `--release` flag is highly encouraged as the GIF rendering performance is very superior compared to debug builds.

The `include-svgs` and `include-fonts` features come enabled by default, these can be disabled by building with the `--no-default-features`. If disabled, paths to fonts and svgs will need to be provided via CLI arguments. If you wish to use a different font or piece set, instead of compiling with `--no-default-features` and relying on CLI arguments, consider adding them to the `svgs/` and `fonts/` directories and compiling with default features enabled.

## Features

### Player bars

If available, at the top and bottom of the GIF we include the player's username, elo, and turn clocks. This feature can be disabled by passing `--no-player-bars`.

### Clocks and real time

If the chess PGN contains `%clk` comments, c2g will attempt to parse them into durations to try and estimate the time taken per move. The clock for each turn is included in the [player bars](#Player bars). Moreover,  we can ask c2g to use the real duration as the delay between GIF frames, with the `--delay="real"`option. This is particularly exciting for bullet games that usually last 1-2 minutes or less.

### Termination circles

The last frame of the gif will draw a small circle over each king to show the result of the game. Some terminations have special circles to indicate the reason why the game ended. Since there are many possible reasons to terminate a chess game, we make use of the Termination PGN header to try to narrow down the cause. If the header is not available, or we cannot find any reason in it, we make the assumption that the losing side resigned for the purpose of choosing what circle to draw. For now, all possible draws are treated the same for the purpose of which circle will be drawn.

This feature can be disabled with `--no-termination`.

## License

Any file in this project that is not listed as an exception is licensed under the [GNU General Public License 3](LICENSE).

The following (free) exceptions apply:

| Files | Author(s) | License |
| :-- | :-- | :-- |
| svgs/cburnett/*.svg | [Colin M.L. Burnett](https://en.wikipedia.org/wiki/User:Cburnett) | [GPLv2+](https://www.gnu.org/licenses/gpl-2.0.txt) |
| fonts/[roboto.ttf](https://fonts.google.com/specimen/Roboto) | Christian Robertson | [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0) |
| fonts/[oswald.ttf](https://fonts.google.com/specimen/Oswald) | Vernon Adams, Kalapi Gajjar, Cyreal | [Open Font License](https://scripts.sil.org/cms/scripts/page.php?site_id=nrsi&id=OFL) |
