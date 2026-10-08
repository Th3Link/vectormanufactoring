import { useState } from "react";

import { Button } from "@/components/ui/button";

import { emitLocal, pickProjectFile, unsupportedReason } from "./host";

/** Stand-in for the native File menu in the browser demo (there is none
 * there), plus the notice when the browser cannot run the editor. */
export function BrowserBar() {
  const [problem] = useState(unsupportedReason);
  return (
    <div className="flex h-9 shrink-0 items-center gap-1 border-b px-2 text-sm">
      <span className="mr-2 font-medium">Curvyo demo</span>
      <Button size="sm" variant="ghost" onClick={() => emitLocal("new-project")}>
        New
      </Button>
      <Button size="sm" variant="ghost" onClick={pickProjectFile}>
        Open…
      </Button>
      <Button
        size="sm"
        variant="ghost"
        onClick={() =>
          emitLocal("request-pack", { save_as: false, app_version: "web-demo" })
        }
      >
        Save (download)
      </Button>
      {problem ? (
        <span role="alert" className="ml-3 text-destructive">
          {problem}
        </span>
      ) : null}
    </div>
  );
}
