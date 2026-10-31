// Presenton's own pages (upload, outline, presentation) in one view
// document: the root layout's store and toaster around an in-memory route.
import "./globals";
import "@/app/globals.css";
import "katex/dist/katex.min.css";
import "./view.css";
import { useEffect, useState, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { Provider } from "react-redux";
import { store } from "@/store/store";
import { Toaster, notify } from "@/components/ui/sonner";
import UploadRoute from "@/app/(presentation-generator)/upload/page";
import OutlineRoute from "@/app/(presentation-generator)/outline/page";
import PresentationRoute from "@/app/(presentation-generator)/presentation/page";
import { location, subscribe } from "./router";
import { acceptedHomeSeed } from "./backend/homeSeed";
import { deck, setDeck, whenStateOverflows } from "./backend/state";
import { agentModelRequests } from "./host";
import { startHomeDocument } from "./homeDocument";

// The homepage drive sets this before the document loads, then reads the deck
// the opened view actually holds. Absent on a normal open, so the branch is dead.
const viewTest = globalThis as typeof globalThis & {
  __ASTER_VIEW_TEST__?: boolean;
  __asterPresentonTesting?: {
    setOutlines(lines: string[]): void;
    snapshot(): { content: string | null; outlines: string[] | null; seed: unknown; modelRequests: number };
  };
};
if (viewTest.__ASTER_VIEW_TEST__) {
  viewTest.__asterPresentonTesting = {
    setOutlines(lines) {
      if (!deck) throw new Error("deck missing");
      setDeck({ ...deck, outlines: lines });
    },
    snapshot() {
      return {
        content: deck?.content ?? null,
        outlines: deck?.outlines ?? null,
        seed: acceptedHomeSeed,
        modelRequests: agentModelRequests,
      };
    },
  };
}

const ROUTES: Record<string, () => JSX.Element> = {
  "/upload": UploadRoute,
  "/outline": OutlineRoute,
  "/presentation": PresentationRoute,
};

function Route() {
  const { pathname } = useSyncExternalStore(subscribe, location);
  const Page = ROUTES[pathname] ?? UploadRoute;
  return <Page key={pathname} />;
}

function App() {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    void startHomeDocument().then((live) => {
      whenStateOverflows(() => notify.warning("演示文稿较大", "超出 64 KiB 的版面修改不会保存；内容和版式仍会保存。"));
      setReady(true);
      if (!live) notify.warning("未在 Studio 中打开", "生成需要本包的 agent，请从 Studio 打开演示文稿。");
    });
  }, []);
  return ready ? <Route /> : null;
}

createRoot(document.getElementById("root")!).render(
  <Provider store={store}>
    <App />
    <Toaster position="top-center" />
  </Provider>,
);
