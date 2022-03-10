import { createRequestHandler as createRemixRequestHandler } from "@remix-run/server-runtime";
// @ts-expect-error
import { getType } from "mime";

function defaultCacheControl(url, assetsPublicPath = "/build/") {
  if (url.pathname.startsWith(assetsPublicPath)) {
    return "public, max-age=31536000, immutable";
  } else {
    return "public, max-age=600";
  }
}

export function createRequestHandler({
  build,
  mode,
  getLoadContext,
}) {
  let remixHandler = createRemixRequestHandler(build, {}, mode);
  return async (request) => {
    try {
      let loadContext = getLoadContext
        ? await getLoadContext(request)
        : undefined;

      return await remixHandler(request, loadContext);
    } catch (e) {
      console.error(e);

      return new Response("Internal Error", { status: 500 });
    }
  };
}

export async function serveStaticFiles(
  request,
  {
    cacheControl,
    publicDir = "/public",
    assetsPublicPath = "/build/",
  }
) {
  const url = new URL(request.url);

  const headers = new Headers();
  const contentType = getType(url.pathname);
  if (contentType) {
    headers.set("Content-Type", contentType);
  }

  if (typeof cacheControl === "function") {
    headers.set("Cache-Control", cacheControl(url));
  } else if (cacheControl) {
    headers.set("Cache-Control", cacheControl);
  } else {
    headers.set("Cache-Control", defaultCacheControl(url, assetsPublicPath));
  }

  const file = await Deno.readFile(`./some-app${publicDir}${url.pathname}`);

  return new Response(file, { headers });
}

export function createRequestHandlerWithStaticFiles({
  build,
  mode,
  getLoadContext,
  staticFiles = {
    publicDir: "/public",
    assetsPublicPath: "/build/",
  },
}) {
  let remixHandler = createRequestHandler({ build, mode, getLoadContext });

  return async (request) => {
    try {
      return await serveStaticFiles(request, staticFiles);
    } catch (error) {
      if (error.code !== "EISDIR" && error.code !== "ENOENT") {
        throw error;
      }
    }

    return remixHandler(request);
  };
}
