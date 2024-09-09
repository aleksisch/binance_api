# Binance API subset on Rust 
This repo implements subset of Binance API for market data to maintain
local order book. 
## How it works
We subscribe to depth updates (trade updates supported, but not used for now)
WebSocket stream. And maintain local order book according to the algorithm described on 
[Binance](https://binance-docs.github.io/apidocs/futures/en/#how-to-manage-a-local-order-book-correctly).
Once depth book updated it printed to stdout. 
There's some flexibility provided using command line arguments.

## Launch
As an example you can simply type
```

```
