# c2g

Turn your chess PGNs to GIFs!

## Examples

### Basic
```
$ cat example/example.pgn | ./c2g --size 640 --output example/chess.gif
```

Will output the following 640x640 GIF:

![Example](/example/chess.gif)

### Bullet

Bullet games are good candidates for real time delay:
```
$ cat example/example_bullet.pgn | ./c2g --size 640 --delay="real" --output example/chess_bullet.gif
```

![Example-Bullet](/example/chess_bullet.gif)

### Clean

If you prefer a more cleaner look without any [features](#Features), you can disable them:
```
$ cat example/example_no_clock.pgn | ./c2g --size 640 --no-player-bars --no-terminations --output example/chess_clean.gif
```

![Example-Clean](/example/chess_clean.gif)

## Features

### Player bars

At the top and bottom of the gif we include the player's username and elo (if available). This feature can be disabled by passing `--no-player-bars`.

### Clocks and real time

If the chess PGN contains `%clk` comments, c2g will attempt to parse them into durations to try and estimate the time taken per move. The clock for each turn is included in the [player bars](#Player bars). Moreover, if the duration is available, we can ask c2g to use the real duration as the delay between gif frames, with the `--delay="real"`option. This is particularly exciting for bullet games that usually last 1-2 minutes or less.

### Termination circles

The last frame of the gif will draw a small circle over each king to show the result of the game. Some terminations have special circles to indicate the reason why the game ended. Since there are many possible reasons to terminate a chess game, we make use of the Termination PGN header to try to narrow down the cause. If the header is not available, or we cannot find any reason in it, we make the assumption that the losing side resigned for the purpose of chossing what circle to draw. For now, all possible draws are treated the same for the purpose of which circle will be drawn.

This feature can be disabled with `--no-termination`

## License

Any file in this project that is not listed as an exception is licensed under the GNU General Public License 3.

The following (free) exceptions apply:

| Files | Author(s) | License |
| :-- | :-- | :-- |
| svgs/pieces/cburnett/*.svg | [Colin M.L. Burnett](https://en.wikipedia.org/wiki/User:Cburnett) | [GPLv2+](https://www.gnu.org/licenses/gpl-2.0.txt) |
| fonts/roboto.ttf | [Christian Robertson](https://fonts.google.com/specimen/Roboto) | [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0) |
