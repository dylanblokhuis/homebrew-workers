import * as build from "@remix-run/dev/server-build";
import { createRequestHandlerWithStaticFiles } from "./something";

const remixHandler = createRequestHandlerWithStaticFiles({
  build,
  mode: "production",
  getLoadContext: () => ({}),
});

self.onmessage = async (event) => {
  console.log("self.onmessage");
  const request = event.data;
  const response = await remixHandler(new Request(
    request.url,
    {
      ...request,
      headers: new Headers(request.headers)
    }
  ));

  self.postMessage({
    headers: Object.fromEntries(response.headers),
    ok: response.ok,
    redirected: response.redirected,
    status: response.status,
    statusText: response.statusText,
    trailer: response.trailer,
    type: response.type,
    url: request.url,
    body: new Uint8Array(await response.arrayBuffer())
  });
  // self.close();
};