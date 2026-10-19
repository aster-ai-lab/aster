export const PERMISSION_GROUPS: {
  label: string
  officialOnly?: boolean
  rules: { action: string; subject: string; label: string }[]
}[] = [
  {
    label: 'platform.nav.suppliers',
    officialOnly: true,
    rules: [
      { action: 'read', subject: 'PlatformSupplier', label: 'org.perm.rule.readPlatformSupplier' },
      { action: 'create', subject: 'PlatformSupplier', label: 'org.perm.rule.createPlatformSupplier' },
      { action: 'update', subject: 'PlatformSupplier', label: 'org.perm.rule.updatePlatformSupplier' },
      { action: 'delete', subject: 'PlatformSupplier', label: 'org.perm.rule.deletePlatformSupplier' }
    ]
  },
  {
    label: 'org.nav.settings',
    rules: [{ action: 'update', subject: 'Organization', label: 'org.perm.rule.updateOrganization' }]
  },
  {
    label: 'org.perm.group.billing',
    rules: [{ action: 'manage', subject: 'Billing', label: 'org.perm.rule.manageBilling' }]
  },
  {
    label: 'org.nav.audit',
    rules: [{ action: 'read', subject: 'Audit', label: 'org.perm.rule.readAudit' }]
  },
  {
    label: 'org.nav.runs',
    rules: [
      { action: 'read', subject: 'Run', label: 'org.perm.rule.readRun' },
      { action: 'stop', subject: 'Run', label: 'org.perm.rule.stopRun' }
    ]
  },
  {
    label: 'org.nav.capabilities',
    rules: [{ action: 'manage', subject: 'Capability', label: 'org.perm.rule.manageCapability' }]
  },
  {
    label: 'org.nav.repositories',
    rules: [
      { action: 'write', subject: 'Repository', label: 'org.perm.rule.writeRepository' },
      { action: 'review', subject: 'Repository', label: 'org.perm.rule.reviewRepository' },
      { action: 'release', subject: 'Repository', label: 'org.perm.rule.releaseRepository' },
      { action: 'admin', subject: 'Repository', label: 'org.perm.rule.adminRepository' }
    ]
  },
  {
    label: 'org.nav.mail',
    rules: [
      { action: 'read', subject: 'MailRecipient', label: 'org.perm.rule.readMailRecipients' },
      { action: 'read', subject: 'MailMessage', label: 'org.perm.rule.readMail' },
      { action: 'create', subject: 'MailMessage', label: 'org.perm.rule.createMail' },
      { action: 'send', subject: 'MailMessage', label: 'org.perm.rule.sendMail' },
      { action: 'update', subject: 'MailMessage', label: 'org.perm.rule.updateMail' },
      { action: 'delete', subject: 'MailMessage', label: 'org.perm.rule.deleteMail' }
    ]
  },
  {
    label: 'org.nav.schedule',
    rules: [
      { action: 'read', subject: 'ScheduleEvent', label: 'org.perm.rule.readSchedule' },
      { action: 'create', subject: 'ScheduleEvent', label: 'org.perm.rule.createSchedule' },
      { action: 'update', subject: 'ScheduleEvent', label: 'org.perm.rule.updateSchedule' },
      { action: 'delete', subject: 'ScheduleEvent', label: 'org.perm.rule.deleteSchedule' }
    ]
  },
  {
    label: 'org.perm.group.member',
    rules: [
      { action: 'update', subject: 'Member', label: 'org.perm.rule.updateMember' },
      { action: 'delete', subject: 'Member', label: 'org.perm.rule.deleteMember' },
      { action: 'invite', subject: 'Member', label: 'org.perm.rule.inviteMember' },
      { action: 'assign', subject: 'Department', label: 'org.perm.rule.assignDepartment' },
      { action: 'assign', subject: 'Position', label: 'org.perm.rule.assignPosition' },
      { action: 'assign', subject: 'Role', label: 'org.perm.rule.assignRole' }
    ]
  },
  {
    label: 'org.perm.group.join',
    rules: [
      { action: 'configure', subject: 'JoinInvite', label: 'org.perm.rule.configureJoinInvite' },
      { action: 'approve', subject: 'JoinRequest', label: 'org.perm.rule.approveJoin' },
      { action: 'reject', subject: 'JoinRequest', label: 'org.perm.rule.rejectJoin' }
    ]
  },
  {
    label: 'org.perm.group.department',
    rules: [
      { action: 'create', subject: 'Department', label: 'org.perm.rule.createDepartment' },
      { action: 'update', subject: 'Department', label: 'org.perm.rule.updateDepartment' },
      { action: 'delete', subject: 'Department', label: 'org.perm.rule.deleteDepartment' }
    ]
  },
  {
    label: 'org.perm.group.position',
    rules: [
      { action: 'create', subject: 'Position', label: 'org.perm.rule.createPosition' },
      { action: 'update', subject: 'Position', label: 'org.perm.rule.updatePosition' },
      { action: 'delete', subject: 'Position', label: 'org.perm.rule.deletePosition' }
    ]
  },
  {
    label: 'org.nav.roles',
    rules: [
      { action: 'create', subject: 'Role', label: 'org.perm.rule.createRole' },
      { action: 'update', subject: 'Role', label: 'org.perm.rule.updateRole' },
      { action: 'delete', subject: 'Role', label: 'org.perm.rule.deleteRole' },
      { action: 'audit', subject: 'Role', label: 'org.perm.rule.auditRole' }
    ]
  },
  {
    label: 'org.nav.workbench',
    rules: [
      { action: 'read', subject: 'WorkItem', label: 'org.perm.rule.readWork' },
      { action: 'create', subject: 'WorkItem', label: 'org.perm.rule.createWork' },
      { action: 'approve', subject: 'WorkItem', label: 'org.perm.rule.approveWork' },
      { action: 'reject', subject: 'WorkItem', label: 'org.perm.rule.rejectWork' },
      { action: 'cancel', subject: 'WorkItem', label: 'org.perm.rule.cancelWork' }
    ]
  }
]

export const GRANTABLE = PERMISSION_GROUPS.flatMap((group) => group.rules)
