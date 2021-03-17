# c2g

Turn your chess PGNs to GIFs!

## Examples

Running:

```
$ cat example/example.pgn | ./c2g --size 640 --output example/chess.gif
```

Will output the following 640x640 GIF:

![Example](/example/chess.gif)

## License

Any file in this project that is not listed as an exception is licensed under the GNU General Public License 3.

The following (free) exceptions apply:

| Files | Author(s) | License |
| :-- | :-- | :-- |
| pieces/*.svg | [Colin M.L. Burnett](https://en.wikipedia.org/wiki/User:Cburnett) | [GPLv2+](https://www.gnu.org/licenses/gpl-2.0.txt) |
| font/roboto.ttf | [Christian Robertson](https://fonts.google.com/specimen/Roboto) | [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0) |
