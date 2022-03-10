import { installGlobals } from "./globals";

export {
  createRequestHandler,
  createRequestHandlerWithStaticFiles,
  serveStaticFiles,
} from "./worker";

installGlobals();
