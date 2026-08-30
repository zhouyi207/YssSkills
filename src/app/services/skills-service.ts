import { invokeCommand } from "./ipc-client";
import {
  catalogSkillDetailDtoSchema,
  catalogSkillsResponseDtoSchema,
  type CatalogSkillDetailDto,
  type CatalogSkillsResponseDto,
  type SkillIdRequestDto,
} from "@/shared/types/skills";

export const skillsService = {
  listCatalogSkills(): Promise<CatalogSkillsResponseDto> {
    return invokeCommand("list_catalog_skills", catalogSkillsResponseDtoSchema);
  },

  getCatalogSkill(request: SkillIdRequestDto): Promise<CatalogSkillDetailDto> {
    return invokeCommand("get_catalog_skill", catalogSkillDetailDtoSchema, { request });
  },
};
