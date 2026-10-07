import { SharedRegion } from "../src/index.ts";

self.onmessage = (event: MessageEvent<string>) => {
  const region = SharedRegion.open(event.data);
  region.buffer()[0] = 99;
  region.notify();
  region.close();
};
