import { invokeCommand } from "./ipc-client";
import {
  catalogSkillDetailDtoSchema,
  catalogSkillsResponseDtoSchema,
  deleteCatalogSkillsResponseDtoSchema,
  deleteSkillSetsResponseDtoSchema,
  exportCatalogSkillsResponseDtoSchema,
  importLocalSkillsResponseDtoSchema,
  rebuildCatalogIndexResponseDtoSchema,
  scanImportFolderResponseDtoSchema,
  skillSetDtoSchema,
  type CatalogSkillDetailDto,
  type CatalogSkillsResponseDto,
  type DeleteCatalogSkillsRequestDto,
  type DeleteCatalogSkillsResponseDto,
  type DeleteSkillSetsRequestDto,
  type DeleteSkillSetsResponseDto,
  type ExportCatalogSkillsRequestDto,
  type ExportCatalogSkillsResponseDto,
  type ImportLocalSkillsRequestDto,
  type ImportLocalSkillsResponseDto,
  type RebuildCatalogIndexResponseDto,
  type ScanImportFolderRequestDto,
  type ScanImportFolderResponseDto,
  type CreateSkillSetRequestDto,
  type SkillIdRequestDto,
  type SkillSetDto,
  type UpdateSkillSetRequestDto,
} from "@/shared/types/skills";

export const skillsService = {
  listCatalogSkills(): Promise<CatalogSkillsResponseDto> {
    return invokeCommand("list_catalog_skills", catalogSkillsResponseDtoSchema);
  },

  rebuildCatalogIndex(): Promise<RebuildCatalogIndexResponseDto> {
    return invokeCommand("rebuild_catalog_index", rebuildCatalogIndexResponseDtoSchema);
  },

  getCatalogSkill(request: SkillIdRequestDto): Promise<CatalogSkillDetailDto> {
    return invokeCommand("get_catalog_skill", catalogSkillDetailDtoSchema, { request });
  },

  scanImportFolder(request: ScanImportFolderRequestDto): Promise<ScanImportFolderResponseDto> {
    return invokeCommand("scan_import_folder", scanImportFolderResponseDtoSchema, { request });
  },

  importLocalSkills(request: ImportLocalSkillsRequestDto): Promise<ImportLocalSkillsResponseDto> {
    return invokeCommand("import_local_skills", importLocalSkillsResponseDtoSchema, { request });
  },

  exportCatalogSkills(
    request: ExportCatalogSkillsRequestDto,
  ): Promise<ExportCatalogSkillsResponseDto> {
    return invokeCommand("export_catalog_skills", exportCatalogSkillsResponseDtoSchema, {
      request,
    });
  },

  deleteCatalogSkills(
    request: DeleteCatalogSkillsRequestDto,
  ): Promise<DeleteCatalogSkillsResponseDto> {
    return invokeCommand("delete_catalog_skills", deleteCatalogSkillsResponseDtoSchema, {
      request,
    });
  },

  createSkillSet(request: CreateSkillSetRequestDto): Promise<SkillSetDto> {
    return invokeCommand("create_skill_set", skillSetDtoSchema, { request });
  },

  updateSkillSet(request: UpdateSkillSetRequestDto): Promise<SkillSetDto> {
    return invokeCommand("update_skill_set", skillSetDtoSchema, { request });
  },

  deleteSkillSets(request: DeleteSkillSetsRequestDto): Promise<DeleteSkillSetsResponseDto> {
    return invokeCommand("delete_skill_sets", deleteSkillSetsResponseDtoSchema, { request });
  },
};
