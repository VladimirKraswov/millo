import { useCallback, useLayoutEffect, useMemo, type DependencyList } from "react";

// Capture before awaiting; a changed context or unmount invalidates the result.
export function useAsyncScope(dependencies: DependencyList) {
  const scope = useMemo(() => ({ active: true, revision: 0 }), dependencies);
  useLayoutEffect(() => {
    scope.active = true;
    return () => {
      scope.active = false;
      scope.revision += 1;
    };
  }, [scope]);
  return useCallback(() => {
    const revision = scope.revision;
    return () => scope.active && scope.revision === revision;
  }, [scope]);
}
