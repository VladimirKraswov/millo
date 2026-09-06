import { StrictMode, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { DialogHost } from "../../src/components/DialogSurface";
import { QuickSketchPlugin } from "../../src/plugins/quick-sketch/QuickSketchPlugin";
import { GeneratedJobStore } from "../../src/platform/jobs/GeneratedJobStore";
import { JobCreationService } from "../../src/platform/jobs/JobCreationService";
import { tauriImageJobGateway } from "../../src/platform/jobs/tauriImageJobGateway";
import type { CuttingTool } from "../../src/shared/tooling";
import type { SketchJobRequest } from "../../src/shared/sketch";

export async function mount() {
  const catalog: CuttingTool[] = await (
    await fetch("/__test__/sketch-tools")
  ).json();
  const snapshot = { tools: catalog, revision: 1 };
  const tools = { current: () => snapshot, subscribe: () => () => {} };
  const store = new GeneratedJobStore();
  const save = (sourceName: string, source: string) => {
    const anchor = document.createElement("a");
    const url = URL.createObjectURL(
      new Blob([source], { type: "application/octet-stream" }),
    );
    anchor.href = url;
    anchor.download = sourceName;
    anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    return Promise.resolve({
      path: `/test/${sourceName}`,
      bytesWritten: source.length,
    });
  };
  const jobs = new JobCreationService(
    {
      ...tauriImageJobGateway,
      generateSketch: async (request) => {
        const response = await fetch("/__test__/sketch-cam", {
          method: "POST",
          body: JSON.stringify(request),
        });
        if (!response.ok) throw new Error(await response.text());
        return response.json();
      },
      saveSketchProject: (doc: SketchJobRequest) =>
        save(
          "sketch.millo-sketch.json",
          JSON.stringify({ version: 1, document: doc }),
        ),
      save: (job) => save(job.sourceName, job.source),
    },
    store,
  );
  function Harness() {
    const published = useSyncExternalStore(
      store.subscribe,
      store.current,
      store.current,
    );
    return (
      <>
        <QuickSketchPlugin jobs={jobs} tools={tools} initialOpen />
        <output aria-label="Published G-code">
          {published?.job.source ?? ""}
        </output>
      </>
    );
  }
  const root = document.createElement("div");
  document.body.replaceChildren(root);
  createRoot(root).render(
    <StrictMode>
      <DialogHost>
        <Harness />
      </DialogHost>
    </StrictMode>,
  );
}
