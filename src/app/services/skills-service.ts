import { invokeCommand } from "./ipc-client";
import {
  catalogSkillDetailDtoSchema,
  catalogSkillsResponseDtoSchema,
  deleteCatalogSkillsResponseDtoSchema,
  exportCatalogSkillsResponseDtoSchema,
  importLocalSkillsResponseDtoSchema,
  scanImportFolderResponseDtoSchema,
  type CatalogSkillDetailDto,
  type CatalogSkillsResponseDto,
  type DeleteCatalogSkillsRequestDto,
  type DeleteCatalogSkillsResponseDto,
  type ExportCatalogSkillsRequestDto,
  type ExportCatalogSkillsResponseDto,
  type ImportLocalSkillsRequestDto,
  type ImportLocalSkillsResponseDto,
  type ScanImportFolderRequestDto,
  type ScanImportFolderResponseDto,
  type SkillIdRequestDto,
} from "@/shared/types/skills";

export const skillsService = {
  listCatalogSkills(): Promise<CatalogSkillsResponseDto> {
    return invokeCommand("list_catalog_skills", catalogSkillsResponseDtoSchema);
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
};
