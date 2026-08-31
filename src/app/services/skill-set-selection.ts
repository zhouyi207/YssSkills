import type { SkillSetDto } from "@/shared/types/skills";

export function applySkillSetSelection(
  currentSkillIds: ReadonlySet<string>,
  set: SkillSetDto,
  availableSkillIds: ReadonlySet<string>,
) {
  const selectedSkillIds = new Set(currentSkillIds);
  set.skillIds.forEach((skillId) => {
    if (availableSkillIds.has(skillId)) {
      selectedSkillIds.add(skillId);
    }
  });
  return {
    selectedSkillIds,
    activeTab: "item" as const,
  };
}
